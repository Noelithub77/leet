//! Function-case harnesses use captured JSON arguments and the real user function.
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

fn identifier(value: &str) -> Result<&str> {
    ensure!(!value.is_empty() && value.bytes().enumerate().all(|(ix, byte)| byte.is_ascii_alphabetic() || byte == b'_' || (ix > 0 && byte.is_ascii_digit())), "Invalid function name");
    Ok(value)
}
fn arguments(meta: &Value, input: &str) -> Result<Vec<(String, Value)>> {
    ensure!(meta.get("classname").is_none(), "Design-class recording currently requires Python or C++");
    let parameters = meta["params"].as_array().context("Function parameters missing")?;
    let lines: Vec<_> = input.lines().filter(|line| !line.trim().is_empty()).collect();
    ensure!(parameters.len() == lines.len(), "Test input must contain one JSON value per parameter");
    parameters.iter().zip(lines).map(|(parameter, line)| Ok((parameter["type"].as_str().context("Parameter type missing")?.into(), serde_json::from_str(line)?))).collect()
}
fn go_type(kind: &str) -> Result<String> {
    if let Some(inner) = kind.strip_suffix("[]") { return Ok(format!("[]{}", go_type(inner)?)); }
    if let Some(inner) = kind.strip_prefix("list<").and_then(|kind| kind.strip_suffix('>')) { return Ok(format!("[]{}", go_type(inner)?)); }
    Ok(match kind { "integer" => "int", "long" => "int64", "double" => "float64", "boolean" => "bool", "string" => "string", "character" => "byte", _ => bail!("Go function harness does not support metadata type {kind}") }.into())
}
fn go_value(kind: &str, value: &Value) -> Result<Value> {
    if let Some(inner) = kind.strip_suffix("[]").or_else(|| kind.strip_prefix("list<").and_then(|kind| kind.strip_suffix('>'))) {
        return value.as_array().context("Array argument expected")?.iter().map(|value| go_value(inner, value)).collect::<Result<Vec<_>>>().map(Value::Array);
    }
    if kind == "character" {
        let text = value.as_str().context("Character argument expected")?;
        ensure!(text.len() == 1, "Go byte character must contain one byte");
        return Ok(Value::from(text.as_bytes()[0]));
    }
    Ok(value.clone())
}
fn output_parameter<'a>(meta: &Value, names: &'a [String]) -> Result<&'a str> {
    let index = meta["output"]["paramindex"].as_u64().unwrap_or_default() as usize;
    names.get(index).map(String::as_str).context("Void function needs an output parameter")
}
pub(super) fn go(meta: &Value, code: &str, input: &str, path: &std::path::Path) -> Result<String> {
    let name = identifier(meta["name"].as_str().context("Function name missing")?)?;
    let parameters = arguments(meta, input)?;
    let mut declarations = String::new();
    let mut names = Vec::new();
    for (ix, (kind, value)) in parameters.iter().enumerate() {
        let value = go_value(kind, value)?;
        let kind = go_type(kind)?; let encoded = serde_json::to_string(&value.to_string())?;
        declarations.push_str(&format!("var a{ix} {kind}; if err := leetjson.Unmarshal([]byte({encoded}), &a{ix}); err != nil {{ panic(err) }}\n"));
        names.push(format!("a{ix}"));
    }
    let result = if meta["return"]["type"] == "void" {
        format!("{name}({}); var result interface{{}} = {}", names.join(","), output_parameter(meta, &names)?)
    } else { format!("result := {name}({})", names.join(",")) };
    let imports = "import leetjson \"encoding/json\"\nimport leetfmt \"fmt\"\n";
    let (prefix, body, line) = if code.trim_start().starts_with("package ") {
        let (package, rest) = code.split_once('\n').context("Go package must be followed by a newline")?;
        (format!("{package}\n{imports}"), rest, 2)
    } else { (format!("package main\n{imports}"), code, 1) };
    Ok(format!("{prefix}//line {}:{line}\n{body}\n//line leet_driver.go:1\nfunc main() {{\n{declarations}{result}\nencoded, err := leetjson.Marshal(result); if err != nil {{ panic(err) }}\nleetfmt.Printf(\"\\n__LEET_TRACE_RESULT_91b6__\\n%s\\n\", encoded)\n}}\n", path.display()))
}
fn java_type(kind: &str) -> Result<String> {
    if let Some(inner) = kind.strip_suffix("[]") { return Ok(format!("{}[]", java_type(inner)?)); }
    if let Some(inner) = kind.strip_prefix("list<").and_then(|kind| kind.strip_suffix('>')) {
        let inner = match inner { "integer" => "Integer".into(), "long" => "Long".into(), "double" => "Double".into(), "boolean" => "Boolean".into(), "character" => "Character".into(), other => java_type(other)? };
        return Ok(format!("java.util.List<{inner}>"));
    }
    Ok(match kind { "integer" => "int", "long" => "long", "double" => "double", "boolean" => "boolean", "string" => "String", "character" => "char", _ => bail!("Java function harness does not support metadata type {kind}") }.into())
}
fn java_literal(kind: &str, value: &Value) -> Result<String> {
    if value.is_null() { return Ok("null".into()); }
    if let Some(inner) = kind.strip_suffix("[]") {
        let values = value.as_array().context("Array argument expected")?.iter().map(|value| java_literal(inner, value)).collect::<Result<Vec<_>>>()?;
        return Ok(format!("new {}{{{}}}", java_type(kind)?, values.join(",")));
    }
    if let Some(inner) = kind.strip_prefix("list<").and_then(|kind| kind.strip_suffix('>')) {
        let values = value.as_array().context("List argument expected")?.iter().map(|value| java_literal(inner, value)).collect::<Result<Vec<_>>>()?;
        return Ok(format!("new java.util.ArrayList<>(java.util.Arrays.asList({}))", values.join(",")));
    }
    Ok(match kind {
        "long" => format!("{}L", value.as_i64().context("Integer argument expected")?),
        "character" => { let text = value.as_str().context("Character argument expected")?; ensure!(text.chars().count() == 1, "Character argument must contain one character"); format!("{}.charAt(0)", serde_json::to_string(text)?) }
        "integer" | "double" => { ensure!(value.is_number(), "Number argument expected"); value.to_string() }
        "boolean" => { ensure!(value.is_boolean(), "Boolean argument expected"); value.to_string() }
        "string" => serde_json::to_string(value.as_str().context("String argument expected")?)?,
        _ => bail!("Unsupported Java argument type {kind}"),
    })
}
pub(super) fn java(meta: &Value, code: &str, input: &str) -> Result<String> {
    let name = identifier(meta["name"].as_str().context("Function name missing")?)?;
    let parameters = arguments(meta, input)?;
    let mut declarations = String::new(); let mut names = Vec::new();
    for (ix, (kind, value)) in parameters.iter().enumerate() {
        declarations.push_str(&format!("{} a{ix} = {};\n", java_type(kind)?, java_literal(kind, value)?)); names.push(format!("a{ix}"));
    }
    let call = format!("new Solution().{name}({})", names.join(","));
    let result = if meta["return"]["type"] == "void" { format!("{call}; Object result = {};", output_parameter(meta, &names)?) } else { format!("Object result = {call};") };
    let source = code.replace("public class Solution", "class Solution");
    // Prelude stays on the first line, preserving all user line numbers.
    Ok(format!("import java.util.*; import java.math.*; {source}\nclass Main {{\npublic static void main(String[] args) throws Exception {{\n{declarations}{result}\nSystem.out.println(\"\\n__LEET_TRACE_RESULT_91b6__\\n\" + encode(result));\n}}\n{}\n}}\n", include_str!("java_encode.txt")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn function_harnesses_preserve_source_and_use_typed_arguments() {
        let meta = serde_json::json!({"name":"sum","params":[{"type":"integer[]"}],"return":{"type":"integer"}});
        let source = go(&meta, "func sum(a []int) int { return a[0] }", "[3,4]", std::path::Path::new("/tmp/solution.go")).unwrap();
        assert!(source.contains("//line /tmp/solution.go:1")); assert!(source.contains("var a0 []int"));
        let source = java(&meta, "class Solution { int sum(int[] a) { return a[0]; } }", "[3,4]").unwrap();
        assert!(source.contains("int[] a0 = new int[]{3,4}")); assert!(source.contains("new Solution().sum(a0)"));
    }
}
