use std::path::Path;
use anyhow::{Context, Result, bail};
use serde_json::Value;
fn identifier(value: &str) -> Result<&str> {
    if value.is_empty() || !value.bytes().enumerate().all(|(i,c)| c.is_ascii_alphabetic() || c == b'_' || (i > 0 && c.is_ascii_digit())) { bail!("Invalid C++ identifier {value:?}"); }
    Ok(value)
}
fn kind(value: &str) -> Result<String> {
    let value = value.trim();
    if let Some(inner) = value.strip_suffix("[]") { return Ok(format!("vector<{}>", kind(inner)?)); }
    if let Some(inner) = value.strip_prefix("list<").and_then(|v| v.strip_suffix('>')) { return Ok(format!("vector<{}>", kind(inner)?)); }
    Ok(match value { "integer" => "int", "long" => "long long", "double" => "double", "boolean" => "bool", "string" => "string", "character" => "char", "ListNode" => "ListNode*", "TreeNode" => "TreeNode*", "void" => "void", _ => bail!("Unsupported C++ metadata type: {value}") }.into())
}
fn params(meta: &Value, input: &str, prefix: &str) -> Result<(String,String)> {
    let mut declarations = String::new(); let mut names = Vec::new();
    for (i,p) in meta["params"].as_array().into_iter().flatten().enumerate() {
        let ty = kind(p["type"].as_str().context("Parameter type missing")?)?;
        if ty == "void" { bail!("Unsupported C++ parameter type: void"); }
        let name = format!("{prefix}{i}"); let read = if input.is_empty() { "judge_driver::read()".into() } else { format!("{input}.at({i})") };
        declarations.push_str(&format!("{ty} {name}=judge_driver::Decode<{ty}>::get({read});\n")); names.push(name);
    }
    Ok((declarations,names.join(",")))
}
pub fn source(meta: &Value, solution_path: &Path, solution_code: &str) -> Result<String> {
    if !solution_path.is_absolute() { bail!("C++ solution path must be absolute"); }
    let path = serde_json::to_string(&solution_path.to_string_lossy())?;
    let mut body = String::new();
    if meta.get("classname").is_some() || meta["systemdesign"].as_bool() == Some(true) {
        let class = identifier(meta["classname"].as_str().context("Design classname missing")?)?;
        let constructor = meta.get("constructor").context("Design constructor metadata missing")?;
        let (declarations,args) = params(constructor,"calls_args.items.at(0).items","a")?;
        body.push_str(&format!("auto calls=judge_driver::read();auto calls_args=judge_driver::read();\n{declarations}{class} instance({args});\nvector<string> results={{\"null\"}};\nfor(size_t call=1;call<calls.items.size();++call){{auto &arguments=calls_args.items.at(call).items;auto &name=calls.items[call].scalar;\n"));
        // Empty argument constructor must avoid C++'s function declaration syntax.
        if args.is_empty() { body = body.replace(&format!("{class} instance();"), &format!("{class} instance;")); }
        for (i,method) in meta["methods"].as_array().context("Design methods metadata missing")?.iter().enumerate() {
            let name = identifier(method["name"].as_str().context("Method name missing")?)?;
            let (declarations,args) = params(method,"arguments","m")?;
            let ret = kind(method["return"]["type"].as_str().unwrap_or("void"))?;
            body.push_str(&format!("{}if(name=={}){{\n{declarations}",if i==0 {""} else {"else "},serde_json::to_string(name)?));
            body.push_str(&if ret == "void" { format!("instance.{name}({args});results.push_back(\"null\");}}\n") } else { format!("results.push_back(judge_driver::dump(instance.{name}({args})));}}\n") });
        }
        body.push_str("else throw runtime_error(\"Unknown design method: \"+name);}\nstring result=\"[\";for(size_t i=0;i<results.size();++i){if(i)result+=',';result+=results[i];}result+=']';\n");
    } else {
        let name = identifier(meta["name"].as_str().context("Function name missing")?)?;
        let (declarations,args) = params(meta,"","a")?; body.push_str(&declarations);
        let ret = kind(meta["return"]["type"].as_str().context("Return type missing")?)?;
        let output = meta["output"]["paramindex"].as_u64();
        if ret == "void" || output.is_some() { body.push_str(&format!("Solution().{name}({args});\n")); }
        else { body.push_str(&format!("auto returned=Solution().{name}({args});\n")); }
        let expr = if let Some(index) = output {
            if index >= meta["params"].as_array().map_or(0,Vec::len) as u64 { bail!("Output paramindex out of bounds"); }
            format!("judge_driver::dump(a{index})")
        } else if ret == "void" { if args.is_empty() { "string(\"null\")".into() } else { "judge_driver::dump(a0)".into() } }
        else { "judge_driver::dump(returned)".into() };
        body.push_str(&format!("string result={expr};\n"));
    }
    Ok(format!("{}\n#line 1 {path}\n{solution_code}\n#line 1 \"judge_driver.cpp\"\nint main(){{try{{\n{body}cout<<\"\\n__LEET_TRACE_RESULT_91b6__\\n\"<<result<<'\\n';\n}}catch(const exception&e){{cerr<<e.what();return 1;}}}}\n",include_str!("driver.hpp")))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn debugger_cpp_driver_types_and_mapping() {
        for (input,expected) in [("integer[][]","vector<vector<int>>"),("list<list<string>>","vector<vector<string>>"),("ListNode[]","vector<ListNode*>")] { assert_eq!(kind(input).unwrap(),expected); }
        assert!(kind("unknown").unwrap_err().to_string().contains("unknown"));
        let code=source(&serde_json::json!({"name":"f","params":[],"return":{"type":"integer"}}),Path::new("/tmp/a.cpp"),"class Solution {};").unwrap();
        assert!(code.contains("#line 1 \"/tmp/a.cpp\"\nclass Solution {};")); assert!(code.contains("Solution().f()"));
    }
}
