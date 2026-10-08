use std::{process::Command,time::Duration};
use anyhow::{Result,bail};
use serde_json::Value;
use super::{Access,AgentKind,Cancel,Catalog,Detected,Event,Model,Outcome,Request};
use super::transport::{Process,outcome,prompt,usage};
pub fn catalog(agent:&Detected)->Result<Catalog>{
    match catalog_models(agent) {Err(error) if error.to_string().contains("Authentication required")=>Ok(Catalog {agent:AgentKind::Cursor,models:Vec::new(),default_model:None,sign_in_hint:Some("Cursor Agent is not signed in; run `cursor-agent login`".into())}),result=>result}
}
fn catalog_models(agent:&Detected)->Result<Catalog>{
    let mut process=Process::spawn(Command::new(&agent.path).arg("models"),Duration::from_secs(30))?;process.close_input();let mut models=Vec::new();let mut default=None;
    while let Some(line)=process.line(&Cancel::default())?{
        let plain=strip_ansi(&line);let line=plain.trim();
        if let Some((id,label))=line.split_once(" - ").or_else(||line.split_once('\t')){let id=id.trim().trim_start_matches('*').trim();if id.contains(' ') || id.is_empty(){continue;}
            if label.to_lowercase().contains("default")||label.to_lowercase().contains("current"){default=Some(id.to_owned());}models.push(Model{id:id.into(),label:label.into(),description:String::new(),efforts:Vec::new(),default_effort:None,fast:false,free:false});}
    }
    if models.is_empty(){bail!("Cursor returned no models; run `cursor-agent login`: {}",process.error());}
    Ok(Catalog{agent:AgentKind::Cursor,models,default_model:default,sign_in_hint:None})
}
fn strip_ansi(text:&str)->String{let mut result=String::new();let mut chars=text.chars();while let Some(c)=chars.next(){if c=='\u{1b}' {if chars.next()==Some('['){for c in chars.by_ref(){if ('@'..='~').contains(&c){break;}}}}else{result.push(c);}}result}
pub fn run(request:&Request,events:&mut dyn FnMut(Event),cancel:&Cancel)->Result<Outcome>{
    let mut command=Command::new(&request.agent.path);command.current_dir(&request.cwd).args(["--print","--output-format","stream-json","--stream-partial-output","--model",&request.selection.model,"--sandbox",if request.access==Access::Full {"disabled"}else{"enabled"}]);
    if request.access==Access::ReadOnly{command.args(["--mode","ask"]);}else{command.arg("--force");}
    if let Some(session)=&request.resume{command.args(["--resume",session]);}command.arg(prompt(request));
    let mut process=Process::spawn(&mut command,Duration::from_secs(900))?;process.close_input();let mut text=String::new();let mut session=None;
    while let Some(line)=process.line(cancel)?{let Ok(value)=serde_json::from_str::<Value>(&line)else{continue};match value["type"].as_str().unwrap_or(""){
        "system"=>{session=value["session_id"].as_str().map(String::from);events(Event::Started{session:session.clone()});}
        "assistant"=>{if let Some(content)=value["message"]["content"].as_array(){for block in content{if let Some(delta)=block["text"].as_str(){text.push_str(delta);events(Event::Text(delta.into()));}}}}
        "result"=>{if value["is_error"]==true{bail!("Cursor: {}",value["result"]);}usage(&value,events);if let Some(result)=value["result"].as_str(){text=result.into();}return outcome(request,text,session,None);}
        _=>{},
    }}bail!("Cursor closed without a result; run `cursor-agent login`: {}",process.error())
}
