use std::{process::Command,time::Duration};
use anyhow::{Result,bail};
use serde_json::Value;
use super::{Access,AgentKind,Cancel,Catalog,Detected,Event,Model,Outcome,Request,ToolKind,transport::{Process,outcome,prompt,usage,extract}};
fn models(text: &str) -> Catalog {
    let models:Vec<_>=text.lines().filter_map(|line|line.split_once('\t')).map(|(id,label)|Model {id:id.trim().into(),label:label.trim().into(),description:String::new(),efforts:Vec::new(),default_effort:None,fast:false,free:false}).collect();
    let default=models.iter().find(|m|m.id.to_lowercase().contains("flash")).or_else(||models.first()).map(|m|m.id.clone());
    Catalog {agent:AgentKind::Antigravity,models,default_model:default,sign_in_hint:None}
}
pub fn catalog(agent: &Detected) -> Result<Catalog> {
    let mut process=Process::spawn(Command::new(&agent.path).arg("models"),Duration::from_secs(30))?; process.close_input(); let mut text=String::new();
    while let Some(line)=process.line(&Cancel::default())? {text.push_str(&line);text.push('\n');}
    let catalog=models(&text); if catalog.models.is_empty() {bail!("Antigravity returned no models: {}",process.error());} Ok(catalog)
}
pub fn run(request: &Request, events: &mut dyn FnMut(Event), cancel: &Cancel) -> Result<Outcome> {
    let mut command=Command::new(&request.agent.path); command.current_dir(&request.cwd).args(["-p",&prompt(request),"--output-format","stream-json","--model",&request.selection.model]);
    if let Some(effort)=&request.selection.effort {command.args(["--effort",effort]);}
    if let Some(schema)=&request.schema {command.arg("--json-schema").arg(schema.to_string());}
    if let Some(session)=&request.resume {command.args(["--conversation",session]);}
    if request.access==Access::ReadOnly {command.args(["--mode","plan"]);} else {command.args(["--mode","accept-edits","--sandbox","--dangerously-skip-permissions"]);}
    let mut process=Process::spawn(&mut command,Duration::from_secs(900))?; process.close_input(); let mut text=String::new();let mut session=None;
    while let Some(line)=process.line(cancel)? {
        let Ok(value)=serde_json::from_str::<Value>(&line) else {continue};
        match value["event"].as_str().unwrap_or("") {
            "init"=>{session=value["conversation_id"].as_str().map(String::from);events(Event::Started {session:session.clone()});}
            "step_update"=>{let step=&value["step_update"];if step["step_type"]=="agent_response" {if let Some(delta)=step["text_delta"].as_str(){text.push_str(delta);events(Event::Text(delta.into()));}}else{events(Event::Tool {title:step["tool_name"].as_str().or_else(||step["step_type"].as_str()).unwrap_or("Tool").into(),kind:match step["tool_name"].as_str(){Some("run_command"|"send_command_input")=>ToolKind::Command,Some("write_to_file"|"replace_file_content"|"multi_replace_file_content")=>ToolKind::Edit,Some("view_file"|"list_dir")=>ToolKind::Read,Some("grep_search"|"find_by_name")=>ToolKind::Search,_=>ToolKind::Other},done:step["state"]=="DONE"});}}
            "result"=>{let result=&value["result"];if result["status"]!="SUCCESS"{bail!("Antigravity: {result}");}usage(result,events);if let Some(response)=result["response"].as_str(){text=response.into();}let structured=request.schema.as_ref().and_then(|_|extract(&text)).or_else(||result.get("structured_output").cloned());return outcome(request,text,session,structured);}
            "error"=>bail!("Antigravity: {value}"),_=>{},
        }
    } bail!("Antigravity closed without a result: {}",process.error())
}
#[cfg(test)] mod tests {use super::*;#[test]fn agents_agy_fixture(){let catalog=models(include_str!("fixtures/agy.txt"));assert!(catalog.default_model.unwrap().contains("flash"));assert!(catalog.models.iter().any(|m|m.id.ends_with("-low")));}}

#[cfg(test)] mod result_tests {
    use super::*;
    #[test] fn agents_agy_answer_precedes_finish_metadata() {
        let result:Value=serde_json::from_str(include_str!("fixtures/agy-result.json")).unwrap();
        assert_eq!(extract(result["response"].as_str().unwrap()),Some(serde_json::json!({"answer":"pong"})));
    }
}
