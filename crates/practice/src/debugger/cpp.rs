use std::{fs, path::Path, process::Command, time::Duration};
use anyhow::{Result, bail};
use super::{Limits, Trace, cpp_driver, python::{Scratch, execute}};
pub fn available() -> Result<(), String> {
    let dir = Scratch::new().map_err(|e| e.to_string())?;
    for (tool,args) in [("g++",vec!["--version"]),("gdb",vec!["-nx","-batch","-ex","python print(1)"])] {
        let mut cmd=Command::new(tool); cmd.args(args);
        if !matches!(execute(&mut cmd,"",Duration::from_secs(3),&dir.0),Ok((true,false,_,_))) { return Err(format!("{tool} unavailable (gdb requires Python support)")); }
    }
    Ok(())
}
pub fn record(solution: &Path, meta: &serde_json::Value, case: &crate::runner::Case, limits: Limits) -> Result<Trace> {
    let dir=Scratch::new()?; let solution=fs::canonicalize(solution)?;
    let source=dir.0.join("main.cpp"); let binary=dir.0.join("solution");
    fs::write(&source,cpp_driver::source(meta,&solution,&fs::read_to_string(&solution)?)?)?;
    let mut cmd=Command::new("g++"); cmd.args(["-std=c++20","-O0","-g","-fno-omit-frame-pointer","-o"]).arg(&binary).arg(&source);
    let (success,timeout,_,stderr)=execute(&mut cmd,"",Duration::from_secs(60),&dir.0)?;
    if !success { let tail=stderr.chars().rev().take(8000).collect::<String>().chars().rev().collect::<String>(); bail!("C++ compilation {}: {tail}",if timeout {"timed out"} else {"failed"}); }
    let events=dir.0.join("steps.jsonl"); let result=dir.0.join("result.json"); let output=dir.0.join("program.stdout"); let error=dir.0.join("program.stderr"); let input=dir.0.join("case.input");
    fs::write(&input,format!("{}\n",case.input.trim_end()))?;
    let spec=serde_json::json!({"solution":solution,"events":events,"result":result,"stdout":output,"stderr":error,"input":input,"max_steps":limits.max_steps,"max_items":limits.max_items});
    let script=dir.0.join("recorder.py");
    fs::write(&script,format!("import json\nSPEC=json.loads({})\n{}",serde_json::to_string(&spec.to_string())?,include_str!("gdb_recorder.py")))?;
    let mut cmd=Command::new("gdb"); cmd.args(["-q","-nx","-batch","-x"]).arg(script).arg("--args").arg(binary);
    let (_,timeout,_,stderr)=execute(&mut cmd,"",limits.timeout,&dir.0)?;
    let mut trace=Trace { language:crate::language::Language::Cpp,case_id:case.id,steps:Vec::new(),truncated:false,output:None,error:None,stdout:String::new() };
    if let Ok(data)=fs::read_to_string(events) { for line in data.lines() { trace.steps.push(serde_json::from_str(line)?); } }
    if let Ok(data)=fs::read_to_string(result) {
        let result:serde_json::Value=serde_json::from_str(&data)?;
        trace.truncated=result["truncated"].as_bool().unwrap_or(false); trace.error=result["error"].as_str().map(str::to_owned);
    }
    let stdout=fs::read_to_string(output).unwrap_or_default();
    if let Some((before,after))=stdout.split_once("\n__LEET_TRACE_RESULT_91b6__\n") { trace.stdout=before.into(); trace.output=Some(after.trim_end().into()); }
    else { trace.stdout=stdout; }
    let program_error=fs::read_to_string(error).unwrap_or_default();
    if timeout { trace.error=Some("Recording timed out".into()); }
    else if trace.error.is_none() && !program_error.is_empty() { trace.error=Some(program_error); }
    else if trace.error.is_none() && trace.output.is_none() && !trace.truncated { trace.error=Some(format!("Debugger did not produce a result: {stderr}")); }
    Ok(trace)
}

#[cfg(test)]
#[path="recorder_tests.rs"]
mod tests;
