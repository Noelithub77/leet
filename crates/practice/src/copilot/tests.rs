use super::*;
use lsp_types::request::Request as _;

#[test]
fn quota_details_distinguish_reported_usage_from_missing_values() {
    let quota: Quota = serde_json::from_value(json!({"copilotPlan":"pro","completions":{"unlimited":true},"premium_interactions":{"percentRemaining":80,"resetDate":"2026-11-01"}})).unwrap();
    assert_eq!(quota.detail(), "Plan · pro\nCompletions · unlimited\nPremium · 20.0% used · 80.0% remaining\nResets · 2026-11-01");
    let quota: Quota = serde_json::from_value(json!({"copilotPlan":"pro","completions":{"percentRemaining":-1}})).unwrap();
    assert_eq!(quota.detail(), "Plan · pro\nUsage not reported by Copilot");
}

#[test]
fn predictions_validate_utf16_ranges_and_extract_only_insertions() {
    let source = "😀value = pri";
    let item = Item { insert_text: "print(1)\r\n".into(), range: Some(lsp::Range::new(lsp::Position::new(0, 10), lsp::Position::new(0, 13))), command: None, extra: Default::default() };
    let suggestion = Suggestion::from_item(source, source.len(), item).unwrap();
    assert_eq!(suggestion.text(), "nt(1)\n");
    assert_eq!(suggestion.item.insert_text, "print(1)\r\n", "Feedback keeps the original item");
    assert!(suggestion.matches(source, source.len()));
    assert!(!suggestion.matches("😀value = pro", source.len()));
    assert!(!suggestion.matches(source, source.len() - 1));
    assert_eq!(byte_offset(source, lsp::Position::new(0, 1)), None, "surrogate midpoint is invalid");
    assert_eq!(position("α\n😀x", "α\n😀".len()), lsp::Position::new(1, 2));
    let modifying = Item { insert_text: "other".into(), range: Some(lsp::Range::new(lsp::Position::new(0, 10), lsp::Position::new(0, 13))), command: None, extra: Default::default() };
    assert!(Suggestion::from_item(source, source.len(), modifying).is_none());
    let source = "print()";
    let item = Item { insert_text: "print(1)".into(), range: Some(lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(0, 7))), command: None, extra: Default::default() };
    assert_eq!(Suggestion::from_item(source, 6, item).unwrap().text(), "1");
    let item: Item = serde_json::from_value(json!({"insertText":"nt(1)","command":{"command":"github.copilot.didAcceptCompletionItem","arguments":["id"]},"opaque":"preserve"})).unwrap();
    assert_eq!(item.command.as_ref().unwrap().title, "");
    assert_eq!(serde_json::to_value(item).unwrap()["opaque"], "preserve");
}

#[cfg(unix)]
#[test]
fn official_protocol_sign_in_document_sync_and_acceptance() {
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("server.py");
    let log = root.path().join("events.jsonl");
    std::fs::write(&script, r#"
import sys,json
signed_in=False
sign_out_attempts=0
def send(value):
    data=json.dumps({'jsonrpc':'2.0',**value}).encode()
    sys.stdout.buffer.write(b'Content-Length: '+str(len(data)).encode()+b'\r\n\r\n'+data)
    sys.stdout.buffer.flush()
while True:
    line=sys.stdin.buffer.readline()
    if not line: break
    length=int(line.split(b':')[1])
    while sys.stdin.buffer.readline().strip(): pass
    request=json.loads(sys.stdin.buffer.read(length))
    with open(sys.argv[1],'a') as log: log.write(json.dumps(request)+'\n')
    method=request['method']; result=None
    if method=='initialize': result={'capabilities':{}}
    elif method=='initialized':
        send({'method':'didChangeStatus','params':{'kind':'Error','message':'Sign in'}})
    elif method=='signIn':
        if not isinstance(request.get('params'),dict):
            send({'id':request['id'],'error':{'code':-32602,'message':'Expected object'}})
            continue
        result={'userCode':'TEST-CODE','command':{'title':'Sign in','command':'github.copilot.finishDeviceFlow','arguments':[]}}
    elif method=='checkStatus':
        result={'status':'OK' if signed_in else 'NotSignedIn','user':'fixture-user' if signed_in else None}
    elif method=='signOut':
        assert request.get('params')=={}
        sign_out_attempts+=1
        if sign_out_attempts==1:
            send({'id':request['id'],'error':{'code':-32603,'message':'Fixture sign-out failure'}})
            continue
        signed_in=False
        result={}
        send({'method':'didChangeStatus','params':{'kind':'Error','message':'Sign in'}})
    elif method=='workspace/executeCommand' and request['params']['command']=='github.copilot.finishDeviceFlow':
        signed_in=True
        send({'method':'didChangeStatus','params':{'kind':'Normal','message':'Ready'}})
        send({'method':'copilot/quotaChange','params':{'copilotPlan':'free','completions':{'percentRemaining':42.5,'unlimited':False},'chat':{'unlimited':True}}})
    elif method=='textDocument/inlineCompletion':
        result={'items':[] if request['params']['textDocument']['uri'].endswith(('rewrite.py','stale.py')) else [{'insertText':'nt(1)','command':{'title':'Accept','command':'github.copilot.didAcceptCompletionItem','arguments':['fixture-id']}}]}
    elif method=='textDocument/copilotInlineEdit':
        document=dict(request['params']['textDocument'])
        if document['uri'].endswith('stale.py'): document['version']+=1
        result={'edits':[{'text':'2','textDocument':document,'range':{'start':{'line':0,'character':8},'end':{'line':0,'character':9}},'command':{'title':'Accept','command':'github.copilot.didAcceptCompletionItem','arguments':['next-edit-id']}}]}
    if 'id' in request: send({'id':request['id'],'result':result})
"#).unwrap();
    async_io::block_on(async {
        let client = Client::start_with(Path::new("python3"), vec!["-u".into(), script.to_string_lossy().into(), log.to_string_lossy().into()], root.path()).unwrap();
        let login = client.sign_in().await.unwrap();
        assert_eq!(login.user_code.as_deref(), Some("TEST-CODE"));
        client.finish_sign_in(login.command.unwrap()).await.unwrap();
        let account = client.account().await.unwrap();
        assert!(account.signed_in());
        assert_eq!(account.user.as_deref(), Some("fixture-user"));
        assert_eq!(client.status(), Status::Ready);
        let detail = client.quota().unwrap().detail();
        assert!(detail.contains("Completions · 57.5% used · 42.5% remaining"));
        assert!(detail.contains("Chat · unlimited"));
        let path = root.path().join("main.py");
        let text = "😀\npri";
        let suggestion = client.predict(&path, Language::Python, text, text.len()).await.unwrap().unwrap();
        client.shown(&suggestion);
        client.accepted(suggestion).await.unwrap();
        client.predict(&path, Language::Python, "😀\npr", "😀\npr".len()).await.unwrap();
        client.predict(&root.path().join("next.py"), Language::Python, "pri", 3).await.unwrap();
        client.close_document(&path);
        let rewrite = "value = 1\npass\n";
        let suggestion = client.predict(&root.path().join("rewrite.py"), Language::Python, rewrite, rewrite.len()).await.unwrap().unwrap();
        assert!(suggestion.is_next_edit());
        assert_eq!(suggestion.range(), 8..9);
        assert_eq!(suggestion.text(), "2");
        client.shown(&suggestion); client.accepted(suggestion).await.unwrap();
        assert!(client.predict(&root.path().join("stale.py"), Language::Python, rewrite, rewrite.len()).await.unwrap().is_none(), "Stale next edit document version was accepted");
        let events: Vec<Value> = std::fs::read_to_string(&log).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(events[0]["params"]["capabilities"]["window"]["showDocument"]["support"], true);
        let predictions: Vec<_> = events.iter().filter(|event| event["method"] == InlineRequest::METHOD).collect();
        assert_eq!(predictions[0]["params"]["textDocument"]["version"], 1);
        assert_eq!(predictions[0]["params"]["position"], json!({"line":1,"character":3}));
        assert_eq!(predictions[1]["params"]["textDocument"]["version"], 2);
        assert_eq!(predictions[2]["params"]["textDocument"]["version"], 1);
        assert!(events.iter().any(|event| event["method"] == "textDocument/didClose"));
        let change = events.iter().find(|event| event["method"] == "textDocument/didChange").unwrap();
        assert_eq!(change["params"]["contentChanges"][0]["range"]["end"], json!({"line":1,"character":3}));
        assert!(events.iter().any(|event| event["method"] == "textDocument/didShowCompletion"));
        assert!(events.iter().any(|event| event["method"] == "textDocument/didShowInlineEdit"));
        assert!(events.iter().any(|event| event["params"]["command"] == "github.copilot.didAcceptCompletionItem"));
        assert!(client.sign_out().await.is_err(), "A failed server sign-out must not be reported as success");
        assert!(client.account().await.unwrap().signed_in());
        client.sign_out().await.unwrap();
        let account = client.account().await.unwrap();
        assert!(!account.signed_in());
        assert!(account.user.is_none());
        client.stop();
    });
}
