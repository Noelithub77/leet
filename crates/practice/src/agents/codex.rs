use std::{process::Command, time::Duration};
use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use super::{Access, AgentKind, Cancel, Catalog, Detected, Effort, Event, Model, Outcome, Request, ToolKind, transport::{Process, outcome, prompt}};
#[derive(Deserialize)] #[serde(rename_all="camelCase")] struct ModelPage { data: Vec<RemoteModel>, next_cursor: Option<String> }
#[derive(Deserialize)] #[serde(rename_all="camelCase")] struct RemoteModel {
    id: String, display_name: String, #[serde(default)] description: String, #[serde(default)] hidden: bool,
    #[serde(default)] supported_reasoning_efforts: Vec<RemoteEffort>, default_reasoning_effort: Option<String>,
    #[serde(default)] service_tiers: Vec<Tier>, #[serde(default)] additional_speed_tiers: Vec<String>, #[serde(default)] is_default: bool,
}
#[derive(Deserialize)] #[serde(rename_all="camelCase")] struct RemoteEffort { reasoning_effort: String, description: String }
#[derive(Deserialize)] struct Tier { id: String, #[serde(default)] name: String }
fn models(page: ModelPage) -> (Vec<Model>, Option<String>) {
    let default = page.data.iter().find(|m|!m.hidden && m.id.to_lowercase().contains("luna")).or_else(||page.data.iter().find(|m|m.is_default && !m.hidden)).map(|m|m.id.clone());
    (page.data.into_iter().filter(|m|!m.hidden).map(|m|Model { id:m.id, label:m.display_name, description:m.description, efforts:m.supported_reasoning_efforts.into_iter().map(|e|Effort { id:e.reasoning_effort, label:e.description }).collect(), default_effort:m.default_reasoning_effort, fast:m.additional_speed_tiers.iter().any(|s|s=="fast") || m.service_tiers.iter().any(|t|t.id=="fast" || t.id=="priority" || t.name.eq_ignore_ascii_case("fast")), free:false }).collect(),default)
}
fn connect(agent: &Detected, cancel: &Cancel, timeout: Duration) -> Result<Process> {
    let mut process = Process::spawn(Command::new(&agent.path).arg("app-server"),timeout)?;
    process.rpc("initialize",json!({"clientInfo":{"name":"leet","version":env!("CARGO_PKG_VERSION")}}),cancel,&mut |_,_|Ok(()))?;
    process.send(&json!({"jsonrpc":"2.0","method":"initialized"}))?; Ok(process)
}
pub fn catalog(agent: &Detected) -> Result<Catalog> {
    let cancel=Cancel::default(); let mut process=connect(agent,&cancel,Duration::from_secs(30))?;
    let mut all=Vec::new(); let mut default=None; let mut cursor=None;
    loop { let value=process.rpc("model/list",json!({"cursor":cursor}),&cancel,&mut |_,_|Ok(()))?; let page:ModelPage=serde_json::from_value(value)?; cursor=page.next_cursor.clone(); let (entries,pick)=models(page); if default.is_none() { default=pick; } all.extend(entries); if cursor.is_none() { break; } }
    default=all.iter().find(|m|m.id.to_lowercase().contains("luna")).map(|m|m.id.clone()).or(default);
    let account=process.rpc("account/read",json!({"refreshToken":false}),&cancel,&mut |_,_|Ok(())).ok();
    Ok(Catalog { agent:AgentKind::Codex, models:all, default_model:default, sign_in_hint:account.filter(|v|v["account"].is_null()).map(|_|"Run `codex login`".into()) })
}
fn approval(process: &mut Process, value: &Value, access: &Access) -> Result<()> {
    if let (Some(id),Some(method))=(value.get("id"),value["method"].as_str()) {
        let decision=if *access!=Access::ReadOnly { "accept" } else { "decline" };
        let result=if method=="item/permissions/requestApproval" { json!({"permissions":{},"scope":"turn"}) } else if method.ends_with("requestApproval") { json!({"decision":decision}) } else { process.send(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Unsupported client request"}}))?; return Ok(()); };
        process.send(&json!({"jsonrpc":"2.0","id":id,"result":result}))?;
    } Ok(())
}
pub fn run(request: &Request, events: &mut dyn FnMut(Event), cancel: &Cancel) -> Result<Outcome> {
    let mut process=connect(&request.agent,cancel,Duration::from_secs(900))?;
    let mut params=json!({"model":request.selection.model,"cwd":request.cwd,"approvalPolicy":"never","sandbox":match request.access { Access::ReadOnly => "read-only", Access::Edit => "workspace-write", Access::Full => "danger-full-access" }});
    if request.access==Access::ReadOnly { params["config"]=json!({"features.shell_tool":false,"features.unified_exec":false,"features.code_mode":false,"features.code_mode_host":false,"features.multi_agent":false}); params["developerInstructions"]=json!("Answer from the prompt alone. Do not invoke tools, run commands, create planning artifacts, or edit files."); }
    let method=if let Some(id)=&request.resume { params["threadId"]=json!(id); "thread/resume" } else { "thread/start" };
    let result=process.rpc(method,params,cancel,&mut |p,v|approval(p,v,&request.access))?;
    let session=result["thread"]["id"].as_str().ok_or_else(||anyhow::anyhow!("Codex returned no thread id"))?.to_owned();
    events(Event::Started { session:Some(session.clone()) });
    process.rpc("turn/start",json!({"threadId":session,"input":[{"type":"text","text":prompt(request),"text_elements":[]}],"effort":request.selection.effort,"outputSchema":request.schema,"serviceTier":if request.selection.fast {Some("fast")}else{None}}),cancel,&mut |p,v|approval(p,v,&request.access))?;
    let mut text=String::new();
    while let Some(line)=process.line(cancel)? {
        let Ok(value)=serde_json::from_str::<Value>(&line) else { continue }; approval(&mut process,&value,&request.access)?;
        let params=&value["params"];
        match value["method"].as_str().unwrap_or("") {
            "item/agentMessage/delta" => if let Some(delta)=params["delta"].as_str() { text.push_str(delta); events(Event::Text(delta.into())); },
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => if let Some(delta)=params["delta"].as_str() { events(Event::Thinking(delta.into())); },
            "item/started" | "item/completed" => {
                let item=&params["item"]; let kind=match item["type"].as_str() { Some("commandExecution")=>Some(ToolKind::Command),Some("fileChange")=>Some(ToolKind::Edit),_=>None };
                if let Some(kind)=kind { events(Event::Tool { title:item["command"].as_str().unwrap_or("File change").into(), kind, done:value["method"]=="item/completed" }); }
                if item["type"]=="agentMessage" && let Some(final_text)=item["text"].as_str() { text=final_text.into(); }
            }
            "thread/tokenUsage/updated" => { let usage=&params["tokenUsage"]["last"]; events(Event::Usage { input_tokens:usage["inputTokens"].as_u64().unwrap_or(0),output_tokens:usage["outputTokens"].as_u64().unwrap_or(0) }); }
            "turn/completed" => { if params["turn"]["status"]!="completed" { bail!("Codex turn failed: {}",params["turn"]); } return outcome(request,text,Some(session),None); }
            "error" => bail!("Codex: {}",params["error"]),
            _=>{},
        }
    } bail!("Codex closed before completing the turn: {}",process.error())
}
#[cfg(test)] mod tests { use super::*; #[test] fn agents_codex_live_fixture() { let value:Value=serde_json::from_str(include_str!("fixtures/codex.json")).unwrap(); let (models,default)=models(serde_json::from_value(value["result"].clone()).unwrap()); assert!(default.unwrap().contains("luna")); assert!(models.iter().any(|m|m.fast)); assert!(models.iter().all(|m|!m.efforts.is_empty())); } }

#[cfg(test)] mod live_tests {
    use super::*;
    #[test] #[ignore = "Uses the signed-in Codex account and a temporary workspace"]
    fn agents_codex_edit_and_cancel() {
        let agent=super::super::detect().into_iter().find(|agent|agent.kind==AgentKind::Codex).expect("Codex installed");
        let catalog=catalog(&agent).unwrap(); let model=catalog.default_model.unwrap();
        let cwd=tempfile::tempdir().unwrap();
        let request=Request { agent,selection:super::super::Selection {agent:AgentKind::Codex,model,effort:Some("low".into()),fast:false},prompt:"Create pong.txt in the current directory containing exactly pong. Then reply done.".into(),schema:None,cwd:cwd.path().into(),access:Access::Edit,resume:None };
        let result=run(&request,&mut |_|{},&Cancel::default()).unwrap();
        assert_eq!(std::fs::read_to_string(cwd.path().join("pong.txt")).unwrap().trim(),"pong"); assert!(result.session.is_some());
        let cancel=Cancel::default();let signal=cancel.clone();let started=std::time::Instant::now();
        let mut request=request;request.access=Access::ReadOnly;request.prompt="Reply pong".into();
        let error=run(&request,&mut |event|if matches!(event,Event::Started {..}){signal.cancel();},&cancel).unwrap_err();
        assert!(error.to_string().contains("cancelled"));assert!(started.elapsed()<Duration::from_secs(15));
    }
}
