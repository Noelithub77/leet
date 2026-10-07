use std::{collections::HashMap, process::Command, time::Duration};
use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use super::{Access, AgentKind, Cancel, Catalog, Detected, Effort, Event, Model, Outcome, Request, ToolKind, transport::{Process, outcome, prompt, usage}};
#[derive(Deserialize)] struct Initialization { models: Vec<RemoteModel>, #[serde(default)] account: Option<Value> }
#[derive(Deserialize)] #[serde(rename_all="camelCase")] struct RemoteModel { value:String, display_name:String, #[serde(default)] description:String, #[serde(default)] supports_effort:bool, #[serde(default)] supported_effort_levels:Vec<String>, #[serde(default)] supports_fast_mode:bool }
fn models(init: Initialization) -> Catalog {
    let default=init.models.iter().find(|m|m.value=="haiku").or_else(||init.models.first()).map(|m|m.value.clone());
    Catalog { agent:AgentKind::Claude, default_model:default, sign_in_hint:if init.account.is_none() {Some("Run `claude auth login`".into())}else{None}, models:init.models.into_iter().map(|m|Model { id:m.value,label:m.display_name,description:m.description,efforts:if m.supports_effort {m.supported_effort_levels.into_iter().map(|id|Effort { label:id.clone(),id }).collect()}else{Vec::new()},default_effort:None,fast:m.supports_fast_mode,free:false }).collect() }
}
fn command(agent: &Detected) -> Command { let mut cmd=Command::new(&agent.path); cmd.args(["-p","--input-format","stream-json","--output-format","stream-json","--verbose","--include-partial-messages"]); cmd }
pub fn catalog(agent: &Detected) -> Result<Catalog> {
    let mut process=Process::spawn(&mut command(agent),Duration::from_secs(30))?;
    process.send(&json!({"type":"control_request","request_id":"leet-init","request":{"subtype":"initialize"}}))?;
    while let Some(line)=process.line(&Cancel::default())? { let Ok(value)=serde_json::from_str::<Value>(&line) else {continue}; if value["type"]=="control_response" { if value["response"]["subtype"]=="error" { bail!("Claude initialization: {}",value["response"]); } return Ok(models(serde_json::from_value(value["response"]["response"].clone())?)); } }
    bail!("Claude Code is not signed in or initialization failed; run `claude auth login`: {}",process.error())
}
fn configure(command: &mut Command, request: &Request) {
    command.current_dir(&request.cwd).args(["--model",&request.selection.model,"--permission-prompts","none","--strict-mcp-config","--mcp-config","{\"mcpServers\":{}}"]);
    if let Some(effort)=&request.selection.effort { command.args(["--effort",effort]); }
    if let Some(schema)=&request.schema { command.arg("--json-schema").arg(schema.to_string()); }
    if let Some(session)=&request.resume { command.args(["--resume",session]); }
    let mut settings=json!({"fastMode":request.selection.fast});
    if request.access==Access::ReadOnly { command.args(["--permission-mode","dontAsk","--tools",""]); } else {
        command.args(["--permission-mode","acceptEdits","--allowedTools","Read","Edit","Write","Bash"]);
        settings["sandbox"]=json!({"enabled":true,"autoAllowBashIfSandboxed":true,"allowUnsandboxedCommands":false,"filesystem":{"allowWrite":[request.cwd]}});
    }
    command.arg("--settings").arg(settings.to_string());
}
pub fn run(request: &Request, events: &mut dyn FnMut(Event), cancel: &Cancel) -> Result<Outcome> {
    let mut command=command(&request.agent); configure(&mut command,request);
    let mut process=Process::spawn(&mut command,Duration::from_secs(900))?;
    process.send(&json!({"type":"control_request","request_id":"leet-init","request":{"subtype":"initialize"}}))?;
    let mut session=None; let mut text=String::new(); let mut partial=false; let mut tools=HashMap::new();
    while let Some(line)=process.line(cancel)? {
        let Ok(value)=serde_json::from_str::<Value>(&line) else {continue};
        match value["type"].as_str().unwrap_or("") {
            "control_response" if value["response"]["request_id"]=="leet-init" => {
                if value["response"]["subtype"]=="error" { bail!("Claude initialization failed: {}",value["response"]); }
                process.send(&json!({"type":"user","message":{"role":"user","content":prompt(request)}}))?;
            }
            "control_request" => { let id=&value["request_id"]; process.send(&json!({"type":"control_response","response":{"subtype":"success","request_id":id,"response":{"behavior":"deny","message":"Interactive approval is disabled"}}}))?; }
            "system" if value["subtype"]=="init" => { session=value["session_id"].as_str().map(String::from); events(Event::Started {session:session.clone()}); }
            "stream_event" => {
                let event=&value["event"]; let delta=&event["delta"];
                if let Some(chunk)=delta["text"].as_str() { partial=true; text.push_str(chunk); events(Event::Text(chunk.into())); }
                if let Some(chunk)=delta["thinking"].as_str() { events(Event::Thinking(chunk.into())); }
            }
            "assistant" => { if let Some(blocks)=value["message"]["content"].as_array() { for block in blocks { if !partial && let Some(chunk)=block["text"].as_str() {text.push_str(chunk);events(Event::Text(chunk.into()));}
                if block["type"]=="tool_use" { let title=block["name"].as_str().unwrap_or("Tool").to_owned(); let kind=match title.as_str(){"Bash"=>ToolKind::Command,"Read"=>ToolKind::Read,"Edit"|"Write"|"MultiEdit"=>ToolKind::Edit,"Glob"|"Grep"=>ToolKind::Search,_=>ToolKind::Other}; if let Some(id)=block["id"].as_str(){tools.insert(id.to_owned(),(title.clone(),kind));} events(Event::Tool { title,kind,done:false }); } } } }
            "user" => { if let Some(blocks)=value["message"]["content"].as_array() {for block in blocks {if let Some(id)=block["tool_use_id"].as_str() && let Some((title,kind))=tools.remove(id) {events(Event::Tool {title,kind,done:true});}}} }
            "result" => {
                usage(&value,events); if value["is_error"].as_bool()==Some(true) { bail!("Claude Code: {}; if authentication expired run `claude auth login`",value["result"].as_str().map(String::from).unwrap_or_else(||value["errors"].to_string())); }
                if let Some(id)=value["session_id"].as_str() {session=Some(id.into());}
                if let Some(result)=value["result"].as_str() {text=result.into();}
                let structured=value.get("structured_output").filter(|v|!v.is_null()).cloned();
                if text.is_empty() && let Some(value)=&structured {text=value.to_string();}
                return outcome(request,text,session,structured);
            }
            _=>{},
        }
    } bail!("Claude closed without a result; run `claude auth login` if needed: {}",process.error())
}
#[cfg(test)] mod tests { use super::*; #[test] fn agents_claude_fixture() { let value:Value=serde_json::from_str(include_str!("fixtures/claude.json")).unwrap(); let catalog=models(serde_json::from_value(value["response"]["response"].clone()).unwrap()); assert_eq!(catalog.default_model.as_deref(),Some("haiku")); assert!(catalog.models.iter().any(|m|m.fast && !m.efforts.is_empty())); assert!(catalog.models.iter().find(|m|m.id=="haiku").unwrap().efforts.is_empty()); } }

#[cfg(test)] mod flag_tests {
    use super::*;
    #[test] fn agents_claude_flags_enforce_access_and_fast_opt_in() {
        let request=Request {agent:Detected {kind:AgentKind::Claude,path:"claude".into(),version:None},selection:super::super::Selection {agent:AgentKind::Claude,model:"haiku".into(),effort:None,fast:true},prompt:"pong".into(),schema:Some(json!({"type":"object"})),cwd:std::env::temp_dir(),access:Access::ReadOnly,resume:Some("previous".into())};
        let mut command=command(&request.agent);configure(&mut command,&request);
        let args:Vec<_>=command.get_args().filter_map(|arg|arg.to_str()).collect();
        assert!(args.windows(2).any(|pair|pair==["--tools",""]));assert!(args.windows(2).any(|pair|pair==["--resume","previous"]));
        let settings=args.windows(2).find(|pair|pair[0]=="--settings").unwrap();assert_eq!(serde_json::from_str::<Value>(settings[1]).unwrap()["fastMode"],true);
        let fixture:Value=serde_json::from_str(include_str!("fixtures/claude-fast.json")).unwrap();assert_eq!(fixture["fast_mode_disabled_reason"],"extra_usage_disabled");
    }
}
