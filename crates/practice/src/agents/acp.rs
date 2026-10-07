use std::{sync::{Arc,Mutex,mpsc,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use anyhow::{Result,bail,Context};
use futures::{FutureExt, future::{select,Either}};
use serde::Deserialize;
use serde_json::{Value,json};
use agent_client_protocol::{AcpAgent,AcpAgentConfig,Agent,ConnectionTo};
use agent_client_protocol::schema::{ProtocolVersion,v1::{InitializeRequest,NewSessionRequest,LoadSessionRequest,SetSessionConfigOptionRequest,PromptRequest,ContentBlock,TextContent,SessionNotification,RequestPermissionRequest,RequestPermissionResponse,RequestPermissionOutcome,SelectedPermissionOutcome,PermissionOption,PermissionOptionKind}};
use super::{Access,AgentKind,Cancel,Catalog,Detected,Effort,Event,Model,Outcome,Request,ToolKind,transport::{outcome,prompt}};
#[derive(Deserialize)] #[serde(rename_all="camelCase")] struct Config {id:String,#[serde(default)]category:String,current_value:Option<String>,#[serde(default)]options:Vec<OptionEntry>}
#[derive(Deserialize)] struct OptionEntry {value:Option<String>,#[serde(default)]name:String,#[serde(default)]description:String,#[serde(default)]options:Vec<OptionEntry>}
fn flatten(options: Vec<OptionEntry>) -> Vec<OptionEntry> { options.into_iter().flat_map(|option|if option.value.is_some(){vec![option]}else{flatten(option.options)}).collect() }
fn models(kind:AgentKind,value:&Value) -> Result<Catalog> {
    let configs:Vec<Config>=serde_json::from_value(value.get("configOptions").cloned().unwrap_or_else(||json!([])))?;
    let effort=configs.iter().find(|c|c.category=="thought_level");
    let efforts=effort.map(|c|c.options.iter().filter_map(|o|o.value.clone().map(|id|Effort {id,label:o.name.clone()})).collect::<Vec<_>>()).unwrap_or_default();
    let default_effort=effort.and_then(|c|c.current_value.clone());
    let model=configs.into_iter().find(|c|c.category=="model" || c.id=="model");
    let (entries,current)=if let Some(model)=model {(flatten(model.options),model.current_value)}else{
        let legacy=&value["models"]; let entries=legacy["availableModels"].as_array().map(|models|models.iter().map(|m|OptionEntry {value:m["modelId"].as_str().map(String::from),name:m["name"].as_str().unwrap_or("").into(),description:m["description"].as_str().unwrap_or("").into(),options:Vec::new()}).collect()).unwrap_or_default();(entries,legacy["currentModelId"].as_str().map(String::from))
    };
    let models:Vec<_>=entries.into_iter().filter_map(|o|o.value.map(|id|Model {free:id.to_lowercase().contains("free") || o.name.to_lowercase().contains("free"),id,label:o.name,description:o.description,efforts:efforts.clone(),default_effort:default_effort.clone(),fast:false})).collect();
    let default=match kind {AgentKind::OpenCode=>models.iter().find(|m|m.id.starts_with("opencode/")&&m.free).map(|m|m.id.clone()).or(current),AgentKind::Gemini=>models.iter().find(|m|m.id.to_lowercase().contains("flash")).or_else(||models.first()).map(|m|m.id.clone()),_=>current};
    Ok(Catalog {agent:kind,models,default_model:default,sign_in_hint:None})
}
fn configuration(kind:AgentKind,value:&Value,category:&str,preferred:&str) -> Option<(String,String)> {
    let configs:Vec<Config>=serde_json::from_value(value.get("configOptions")?.clone()).ok()?;
    let option=configs.into_iter().find(|c|c.category==category || c.id==category)?;
    let selected=flatten(option.options).into_iter().find_map(|entry|entry.value.filter(|v|v==preferred || (category=="mode" && preferred=="build" && kind==AgentKind::Gemini && v=="default")))?;
    Some((option.id,selected))
}
fn permission_outcome(options:&[PermissionOption],access:&Access)->RequestPermissionOutcome {
    let wanted=if *access==Access::Edit {PermissionOptionKind::AllowOnce}else{PermissionOptionKind::RejectOnce};
    options.iter().find(|option|option.kind==wanted).map(|option|RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option.option_id.clone()))).unwrap_or(RequestPermissionOutcome::Cancelled)
}
fn protocol_error(error:impl std::fmt::Display)->agent_client_protocol::Error {agent_client_protocol::Error::into_internal_error(std::io::Error::other(error.to_string()))}
enum Reply {Catalog(Catalog),Outcome(Outcome)}
fn execute(agent:Detected,request:Option<Request>,events:&mut dyn FnMut(Event),cancel:&Cancel) -> Result<Reply> {
    let (sender,receiver)=mpsc::channel();let (done_tx,done_rx)=mpsc::channel();let cancel_thread=cancel.clone();
    let timeout=if request.is_none(){Duration::from_secs(30)}else{Duration::from_secs(900)};
    let worker=std::thread::spawn(move || {
        let result=async_io::block_on(async {
            let mut config=AcpAgentConfig::new(&agent.path).arg(if agent.kind==AgentKind::OpenCode {"acp"}else{"--acp"});
            if agent.kind==AgentKind::OpenCode && let Some(request)=&request { let permission=if request.access==Access::ReadOnly {json!({"*":"deny"})}else{json!({"*":"ask","external_directory":"deny"})}; config=config.env("OPENCODE_CONFIG_CONTENT",json!({"permission":permission}).to_string()); }
            // ACP's process transport terminates its entire process group on drop.
            let transport=AcpAgent::new(config);
            let prompting=Arc::new(AtomicBool::new(false));let accepting=prompting.clone();
            let text=Arc::new(Mutex::new(String::new()));let chunks=text.clone();let notifications=sender.clone();
            let access=request.as_ref().map(|r|r.access.clone()).unwrap_or(Access::ReadOnly);
            let result=Arc::new(Mutex::new(None));let completed=result.clone();
            let connection=agent_client_protocol::Client.builder()
                .on_receive_notification(async move |notification:SessionNotification,_cx| {
                    if !accepting.load(Ordering::Acquire) {return Ok(());}
                    let value=serde_json::to_value(&notification.update).map_err(protocol_error)?;
                    match value["sessionUpdate"].as_str().unwrap_or("") {
                        "agent_message_chunk"=>if let Some(delta)=value["content"]["text"].as_str(){if let Ok(mut text)=chunks.lock(){text.push_str(delta);}let _=notifications.send(Event::Text(delta.into()));},
                        "agent_thought_chunk"=>if let Some(delta)=value["content"]["text"].as_str(){let _=notifications.send(Event::Thinking(delta.into()));},
                        "tool_call"|"tool_call_update"=>{let kind=match value["kind"].as_str(){Some("read")=>ToolKind::Read,Some("edit")=>ToolKind::Edit,Some("execute")=>ToolKind::Command,Some("search")=>ToolKind::Search,_=>ToolKind::Other};let _=notifications.send(Event::Tool {title:value["title"].as_str().unwrap_or("Tool").into(),kind,done:value["status"]=="completed" || value["status"]=="failed"});},_=>{},
                    } Ok(())
                },agent_client_protocol::on_receive_notification!())
                .on_receive_request(async move |permission:RequestPermissionRequest,responder,_cx| {
                    let outcome=permission_outcome(&permission.options,&access);
                    responder.respond(RequestPermissionResponse::new(outcome))
                },agent_client_protocol::on_receive_request!())
                .connect_with(transport,move |connection:ConnectionTo<Agent>|async move {
                    let init=connection.send_request(InitializeRequest::new(ProtocolVersion::V1)).block_task().await?;
                    let request_session=if let Some(request)=&request { if let Some(session)=&request.resume {
                        let capabilities=serde_json::to_value(&init.agent_capabilities).map_err(protocol_error)?;
                        if capabilities["loadSession"]!=true {return Err(protocol_error("This agent does not support session resume"));}
                        let load:LoadSessionRequest=serde_json::from_value(json!({"sessionId":session,"cwd":request.cwd,"mcpServers":[]})).map_err(protocol_error)?;
                        let response=connection.send_request(load).block_task().await?;
                        (session.clone(),serde_json::to_value(response).map_err(protocol_error)?)
                    }else{let response=connection.send_request(NewSessionRequest::new(request.cwd.clone())).block_task().await?;let session=response.session_id.to_string();(session,serde_json::to_value(response).map_err(protocol_error)?)}
                    }else{let response=connection.send_request(NewSessionRequest::new(std::env::temp_dir())).block_task().await?;let session=response.session_id.to_string();(session,serde_json::to_value(response).map_err(protocol_error)?)};
                    let (session,config)=request_session;
                    let reply=if let Some(request)=request {
                        let _=sender.send(Event::Started {session:Some(session.clone())});
                        for (category,preferred) in [("model",Some(request.selection.model.as_str())),("mode",Some(if request.access==Access::ReadOnly {"plan"}else{"build"})),("thought_level",request.selection.effort.as_deref())] {
                            if let Some(preferred)=preferred {if let Some((id,value))=configuration(agent.kind,&config,category,preferred){let set:SetSessionConfigOptionRequest=serde_json::from_value(json!({"sessionId":session,"configId":id,"value":value})).map_err(protocol_error)?;connection.send_request(set).block_task().await?;}else if category=="model"{return Err(protocol_error(format!("Agent does not advertise selected model {preferred}")));}}
                        }
                        prompting.store(true,Ordering::Release);
                        let response=connection.send_request(PromptRequest::new(session.clone(),vec![ContentBlock::Text(TextContent::new(prompt(&request)))])).block_task().await?;
                        let response=serde_json::to_value(response).map_err(protocol_error)?;
                        if response["stopReason"]=="cancelled" {return Err(protocol_error("Agent run was cancelled"));}
                        let text=text.lock().map_err(protocol_error)?.clone();
                        Reply::Outcome(outcome(&request,text,Some(session),None).map_err(protocol_error)?)
                    }else{Reply::Catalog(models(agent.kind,&config).map_err(protocol_error)?)};
                    *completed.lock().map_err(protocol_error)?=Some(reply);Ok(())
                }).boxed();
            let watch=async {let start=Instant::now();loop {if cancel_thread.is_cancelled(){return "Agent run was cancelled";}
                if start.elapsed()>timeout{return "Agent timed out";}async_io::Timer::after(Duration::from_millis(50)).await;}}.boxed();
            match select(connection,watch).await {Either::Left((result,_))=>result.map_err(anyhow::Error::from)?,Either::Right((message,_))=>bail!("{message}"),}
            result.lock().map_err(|_|anyhow::anyhow!("Agent result lock poisoned"))?.take().context("Agent closed before returning a result")
        });let _=done_tx.send(result);
    });
    let result=loop {while let Ok(event)=receiver.try_recv(){events(event);}match done_rx.recv_timeout(Duration::from_millis(50)){Ok(result)=>break result,Err(mpsc::RecvTimeoutError::Timeout)=>{},Err(_)=>break Err(anyhow::anyhow!("ACP worker stopped"))}};
    let _=worker.join();while let Ok(event)=receiver.try_recv(){events(event);}result
}
pub fn catalog(agent:&Detected)->Result<Catalog>{
    if agent.kind==AgentKind::Cursor{return super::cursor::catalog(agent);}
    match execute(agent.clone(),None,&mut |_|{},&Cancel::default()) {
        Ok(Reply::Catalog(catalog))=>Ok(catalog),Ok(_)=>bail!("Unexpected ACP result"),
        Err(error) if error.to_string().to_lowercase().contains("api key")=>Ok(Catalog {agent:agent.kind,models:Vec::new(),default_model:None,sign_in_hint:Some(format!("{error} Run `{}` and configure authentication.",agent.kind.binary()))}),
        Err(error)=>Err(error),
    }
}
pub fn run(request:&Request,events:&mut dyn FnMut(Event),cancel:&Cancel)->Result<Outcome>{if request.agent.kind==AgentKind::Cursor{return super::cursor::run(request,events,cancel);}match execute(request.agent.clone(),Some(request.clone()),events,cancel)?{Reply::Outcome(outcome)=>Ok(outcome),_=>bail!("Unexpected ACP result")}}
#[cfg(test)]mod tests{use super::*;#[test]fn agents_acp_fixture(){let value:Value=serde_json::from_str(include_str!("fixtures/opencode.json")).unwrap();let catalog=models(AgentKind::OpenCode,&value["result"]).unwrap();assert_eq!(catalog.default_model.as_deref(),Some("opencode/exo-free"));assert!(catalog.models.iter().find(|m|m.id=="opencode/exo-free").unwrap().free);assert_eq!(configuration(AgentKind::OpenCode,&value["result"],"mode","plan"),Some(("mode".into(),"plan".into())));}}

#[cfg(test)] mod permission_tests {
    use super::*;
    #[test] fn agents_permissions_decline_readonly() {
        let options=vec![PermissionOption::new("yes","Allow",PermissionOptionKind::AllowOnce),PermissionOption::new("no","Deny",PermissionOptionKind::RejectOnce)];
        assert_eq!(serde_json::to_value(permission_outcome(&options,&Access::ReadOnly)).unwrap()["optionId"],"no");
        assert_eq!(serde_json::to_value(permission_outcome(&options,&Access::Edit)).unwrap()["optionId"],"yes");
        assert_eq!(permission_outcome(&options[..1],&Access::ReadOnly),RequestPermissionOutcome::Cancelled);
    }
}

#[cfg(test)] mod effort_tests {
    use super::*;
    #[test] fn agents_acp_thought_level_options() {
        let mut value:Value=serde_json::from_str(include_str!("fixtures/opencode.json")).unwrap();
        value["result"]["configOptions"].as_array_mut().unwrap().push(json!({"id":"thinking","category":"thought_level","currentValue":"low","options":[{"value":"low","name":"Low"},{"value":"high","name":"High"}]}));
        let catalog=models(AgentKind::OpenCode,&value["result"]).unwrap();
        assert!(catalog.models.iter().all(|model|model.efforts.len()==2 && model.default_effort.as_deref()==Some("low")));
        assert_eq!(configuration(AgentKind::OpenCode,&value["result"],"thought_level","high"),Some(("thinking".into(),"high".into())));
    }
}

#[cfg(all(test, unix))]
#[path = "acp_replay_tests.rs"]
mod replay_tests;
