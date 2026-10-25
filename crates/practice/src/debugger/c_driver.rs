//! C function harness for scalar and one-dimensional judge arguments.
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
fn scalar(kind: &str) -> Result<&'static str> {
    Ok(match kind { "integer" => "int", "long" => "long long", "double" => "double", "boolean" => "bool", "character" => "char", "string" => "char*", _ => bail!("C function harness does not support {kind}") })
}
fn literal(kind: &str, value: &Value) -> Result<String> {
    Ok(match kind {
        "string" => serde_json::to_string(value.as_str().context("String argument expected")?)?,
        "character" => { let text = value.as_str().context("Character argument expected")?; ensure!(text.len() == 1, "C character must contain one byte"); format!("{}", text.as_bytes()[0]) }
        "boolean" => { ensure!(value.is_boolean(), "Boolean argument expected"); if value == true { "true" } else { "false" }.into() }
        _ => { ensure!(value.is_number(), "Number argument expected"); value.to_string() }
    })
}
fn print_value(kind: &str, expression: &str) -> Result<String> {
    Ok(match kind {
        "integer" | "long" => format!("printf(\"%lld\", (long long)({expression}));"),
        "double" => format!("printf(\"%.17g\", (double)({expression}));"),
        "boolean" => format!("fputs(({expression}) ? \"true\" : \"false\", stdout);"),
        "string" => format!("leet_string({expression});"),
        "character" => format!("{{char text[2]={{({expression}),0}};leet_string(text);}}"),
        _ => bail!("C function harness cannot encode {kind}"),
    })
}
pub(super) fn source(meta: &Value, code: &str, input: &str, path: &std::path::Path) -> Result<String> {
    ensure!(meta.get("classname").is_none(), "C design-class harnesses are not supported");
    let name = meta["name"].as_str().context("Missing C function name")?;
    ensure!(!name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'), "Invalid C function name");
    let pattern = regex::Regex::new(&format!(r"\b{}\s*\(([^)]*)\)\s*\{{", regex::escape(name)))?;
    let capture = pattern.captures(code).context("Could not find the C function signature")?;
    let signature = capture.get(1).context("C parameters missing")?.as_str();
    let params = meta["params"].as_array().context("Missing function parameters")?;
    let values: Vec<Value> = input.lines().filter(|line| !line.trim().is_empty()).map(serde_json::from_str).collect::<std::result::Result<_, _>>()?;
    ensure!(values.len() == params.len(), "Test input must contain one JSON value per parameter");
    let mut declarations = "int leet_return_size=0;\n".to_owned(); let mut calls = Vec::new(); let mut ix = 0;
    let mut last_array = None;
    let formal_name = regex::Regex::new(r"([A-Za-z_]\w*)\s*(?:\[\s*\])?\s*$")?;
    let mut lengths = Vec::new();
    for formal in signature.split(',').filter(|part| !matches!(part.trim(), "" | "void")) {
        let formal = formal_name.captures(formal).and_then(|capture| capture.get(1)).context("Unsupported C parameter declaration")?.as_str();
        if formal == "returnSize" { calls.push("&leet_return_size".into()); continue; }
        if (formal.ends_with("Size") || formal.ends_with("Len")) && last_array.is_some() {
            calls.push(last_array.take().context("Missing C array length")?); continue;
        }
        let parameter = params.get(ix).context("C signature has extra arguments not described by judge metadata")?;
        let kind = parameter["type"].as_str().context("Missing C parameter type")?;
        let value = &values[ix]; let variable = format!("a{ix}");
        if let Some(inner) = kind.strip_suffix("[]") {
            let items = value.as_array().context("C array argument expected")?;
            let initializers = items.iter().map(|value| literal(inner, value)).collect::<Result<Vec<_>>>()?;
            declarations.push_str(&format!("{} {variable}[{}] = {{{}}};\n", scalar(inner)?, items.len().max(1), if initializers.is_empty() { "0".into() } else { initializers.join(",") }));
            last_array = Some(items.len().to_string()); lengths.push(items.len());
        } else { declarations.push_str(&format!("{} {variable} = {};\n", scalar(kind)?, literal(kind, value)?)); last_array = None; lengths.push(0); }
        calls.push(variable); ix += 1;
    }
    ensure!(ix == params.len(), "C signature does not match the judge parameters");
    let return_kind = meta["return"]["type"].as_str().context("Missing C return type")?;
    let mut body = declarations;
    let (expression, output_kind, count) = if return_kind == "void" {
        body.push_str(&format!("{name}({});\n", calls.join(",")));
        let ix = meta["output"]["paramindex"].as_u64().unwrap_or_default() as usize;
        let kind = params.get(ix).and_then(|parameter| parameter["type"].as_str()).context("Void C function needs an output parameter")?;
        (format!("a{ix}"), kind.to_owned(), lengths.get(ix).copied().unwrap_or_default().to_string())
    } else {
        let kind = if let Some(inner) = return_kind.strip_suffix("[]") { format!("{}*", scalar(inner)?) } else { scalar(return_kind)?.into() };
        body.push_str(&format!("{kind} result = {name}({});\n", calls.join(",")));
        ("result".into(), return_kind.to_owned(), "leet_return_size".into())
    };
    body.push_str("fputs(\"\\n__LEET_TRACE_RESULT_91b6__\\n\",stdout);\n");
    if let Some(inner) = output_kind.strip_suffix("[]") {
        body.push_str(&format!("putchar('[');for(int i=0;i<{count};i++){{if(i)putchar(',');{}}}putchar(']');", print_value(inner, &format!("{expression}[i]"))?));
    } else { body.push_str(&print_value(&output_kind, &expression)?); }
    body.push_str("putchar('\\n');return 0;");
    Ok(format!("#include <stdio.h>\n#include <stdlib.h>\n#include <stdbool.h>\n#include <string.h>\n#line 1 {}\n{code}\n#line 1 \"leet_driver.c\"\nstatic void leet_string(const char*s){{if(!s){{fputs(\"null\",stdout);return;}}putchar('\"');for(;*s;s++){{unsigned char c=*s;if(c=='\"'||c=='\\\\'){{putchar('\\\\');putchar(c);}}else if(c<32)printf(\"\\\\u%04x\",c);else putchar(c);}}putchar('\"');}}\nint main(void){{{body}}}\n", serde_json::to_string(&path.to_string_lossy())?))
}
