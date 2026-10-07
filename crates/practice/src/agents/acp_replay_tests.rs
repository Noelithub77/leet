use super::*;
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn agents_acp_resume_ignores_replayed_answer_and_events() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent");
    fs::write(&path, r#"#!/usr/bin/env python3
import sys,json

def send(value):
 print(json.dumps(value),flush=True)
def message(text):
 send({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'old','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':text}}}})
for line in sys.stdin:
 request=json.loads(line)
 method=request.get('method')
 if method=='initialize':
  result={'protocolVersion':1,'agentCapabilities':{'loadSession':True}}
 elif method=='session/load':
  message('{"stale":true}')
  result={'configOptions':[{'id':'model','name':'Model','type':'select','category':'model','currentValue':'mock','options':[{'value':'mock','name':'Mock'}]}]}
 elif method=='session/set_config_option':
  result={'configOptions':[]}
 elif method=='session/prompt':
  message('{"answer":"corrected"}')
  result={'stopReason':'end_turn'}
 else:
  send({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32601,'message':method}})
  continue
 send({'jsonrpc':'2.0','id':request['id'],'result':result})
"#).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let request = Request { agent: Detected { kind: AgentKind::OpenCode, path, version: None }, selection: super::super::Selection { agent: AgentKind::OpenCode, model: "mock".into(), effort: None, fast: false }, prompt: "correct previous answer".into(), schema: Some(json!({"type":"object"})), cwd: dir.path().into(), access: Access::ReadOnly, resume: Some("old".into()) };
    let mut received = Vec::new();
    let answer = run(&request, &mut |event| received.push(event), &Cancel::default()).unwrap();
    assert_eq!(answer.structured, Some(json!({"answer":"corrected"})));
    assert_eq!(answer.text, "{\"answer\":\"corrected\"}");
    assert!(!received.iter().any(|event| matches!(event, Event::Text(text) if text.contains("stale"))));
}
