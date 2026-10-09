//! Real input against production snippet views; no agents or editor scans run.
use super::*;
use anyhow::{Result,ensure};
use std::{path::Path,sync::Arc};
use gpui_kit::test::TestWindowExt as _;
use practice::{config::Config,db::Db};
use gpui_kit::component::input::CompletionProvider;

struct FixtureCompletions;
impl CompletionProvider for FixtureCompletions {
    fn is_completion_trigger(&self,_:usize,_:&str,_:&mut App)->bool { true }
    fn completions(&self,_:&ropey::Rope,_:usize,_:lsp_types::CompletionContext,_:&mut Window,_:&mut App)->Task<anyhow::Result<lsp_types::CompletionResponse>> {
        Task::ready(Ok(lsp_types::CompletionResponse::Array(vec![
            lsp_types::CompletionItem{label:"local_value".into(),kind:Some(lsp_types::CompletionItemKind::VARIABLE),detail:Some("Language-server fixture".into()),insert_text:Some("local_value".into()),..Default::default()},
            lsp_types::CompletionItem{label:"load_graph".into(),kind:Some(lsp_types::CompletionItemKind::FUNCTION),detail:Some("(path)".into()),..Default::default()},
            lsp_types::CompletionItem{label:"lsp_loop".into(),insert_text:Some("while ${1:ready}:\n    print($1)$0".into()),insert_text_format:Some(lsp_types::InsertTextFormat::SNIPPET),..Default::default()}
        ])))
    }
}

pub(crate) fn native(cx:&mut HeadlessAppContext,output:&Path,pixels:bool)->Result<serde_json::Value>{
    cx.update(|cx|{super::bind_keys(cx);crate::snippets::expansion::bind_keys(cx);cx.bind_keys([KeyBinding::new("ctrl-j",crate::actions::InsertSnippet,None)]);});
    let(handle,pane)=cx.update(|cx|gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:point(px(0.),px(0.)),size:size(px(1000.),px(700.))})),show:false,focus:false,..Default::default()},cx,|window,cx|cx.new(|cx|{
        let state=cx.new(|cx|EditorState::new(window,cx).language("python"));
        let mut pane=crate::snippets::expansion::EditorPane::new(state.clone(),Language::Python,window,cx);
        state.update(cx,|editor,_|editor.lsp_mut().completion_provider=Some(std::rc::Rc::new(FixtureCompletions)));
        pane.library=vec![Snippet{name:"Fixture loop".into(),prefixes:vec!["loop".into(),"!loop".into()],body:"for ${1:i} in ${2:range(n)}:\n\tprint($1)\n$0".into(),description:"A fixture".into(),scope:Some(Language::Python),template:false,origin:Origin::User}];
        pane.library.push(Snippet{name:"Wrap selection".into(),prefixes:vec!["wrap".into()],body:"${TM_SELECTED_TEXT}\n$0".into(),description:"Keep selected code".into(),scope:Some(Language::Python),template:false,origin:Origin::User});
        pane.library.push(practice::snippets::builtin().into_iter().find(|s|s.scope==Some(Language::Python)&&s.prefix()=="bfs").unwrap());
        state.update(cx,|e,cx|e.focus(window,cx));pane
    })))?;
    let handle=handle.into();cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        window.render_frame(cx);window.input("!loop",cx);window.render_frame(cx);window.press("tab",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        let state=pane.read(cx).state.clone();ensure!(state.read(cx).selected_value().as_str()=="i","Tab did not select first placeholder");
        window.input("node",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        let state=pane.read(cx).state.clone();ensure!(state.read(cx).value().contains("print(node)"),"Mirror failed to follow typing");
        window.render_frame(cx);window.press("tab",cx);ensure!(state.read(cx).selected_value().as_str()=="range(n)","Tab did not advance");
        window.render_frame(cx);window.press("shift-tab",cx);ensure!(state.read(cx).selected_value().as_str()=="node","Shift+Tab did not go back");
        window.press("escape",cx);
        state.update(cx,|e,cx|{e.set_value("",window,cx);e.focus(window,cx);});
        window.input("lo",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        window.render_frame(cx);
        let menu=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Unified completion menu hidden"))?;
        ensure!(window.find("source-completions").visible(),"Cursor completion list hidden");
        ensure!(menu.content.items.iter().any(|item|item.label=="loop"),"Snippet missing from normal completions");
        ensure!(menu.content.items.iter().any(|item|item.label=="local_value"),"Language-server suggestions missing from merged menu");
        ensure!(menu.content.items.iter().any(|item|item.documentation.is_some()),"Highlighted snippet preview missing");
        Ok(())
    })??;
    cx.run_until_parked();cx.update_window(handle,|_,window,cx|window.render_frame(cx))?;
    if pixels{cx.capture_screenshot(handle)?.save(output.join("snippet-picker.png"))?;}
    cx.update_window(handle,|_,window,cx|window.press("enter",cx))?;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        let state=pane.read(cx).state.clone();
        ensure!(state.read(cx).selected_value().as_str()=="i","Completion acceptance lost the first placeholder");
        ensure!(!state.read(cx).value().contains("$"),"Completion inserted unexpanded snippet syntax");
        window.input("vertex",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        let state=pane.read(cx).state.clone();
        ensure!(state.read(cx).value().contains("print(vertex)"),"Completion lost linked placeholders");
        window.press("tab",cx);
        ensure!(state.read(cx).selected_value().as_str()=="range(n)","Completion lost Tab navigation");
        window.press("escape",cx);
        state.update(cx,|editor,cx|{editor.set_value("",window,cx);editor.focus(window,cx);});
        window.input("lo",cx);
        Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        let index=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Completion menu missing"))?.content.items.iter().position(|item|item.label=="local_value").unwrap();
        for _ in 0..index{window.press("down",cx);}
        window.press("enter",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        ensure!(pane.read(cx).state.read(cx).value().as_str()=="local_value","Completion duplicated its typed prefix");
        window.press("ctrl-j",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        let index=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Completion menu missing"))?.content.items.iter().position(|item|item.label=="lsp_loop").ok_or_else(||anyhow::anyhow!("LSP snippet missing"))?;
        for _ in 0..index{window.press("down",cx);}
        window.press("tab",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,_,cx|->Result<()>{
        ensure!(pane.read(cx).state.read(cx).selected_value().as_str()=="ready","LSP snippet lost its placeholders");Ok(())
    })??;
    cx.update_window(handle,|_,window,cx|{
        window.press("escape",cx);
        pane.read(cx).state.clone().update(cx,|editor,cx|{editor.set_value("",window,cx);editor.focus(window,cx);});
        window.input("b",cx);
    })?;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        window.render_frame(cx);
        let menu=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Typing did not show snippets"))?;
        let item=menu.content.items.iter().find(|item|item.label=="bfs").ok_or_else(||anyhow::anyhow!("BFS suggestion missing"))?;
        let doc=serde_json::to_string(&item.documentation)?;
        ensure!(doc.contains("Input ·")&&doc.contains("Output ·")&&doc.contains("Example ·"),"Algorithm usage guide missing");
        let bounds=window.find("source-completions").bounds();
        ensure!(bounds.size.width<=pane.read(cx).state.read(cx).input_bounds().size.width&&bounds.size.height<px(300.),"Completion menu exceeds the editor bounds");
        Ok(())
    })??;
    if pixels{cx.capture_screenshot(handle)?.save(output.join("snippet-bfs-help.png"))?;}
    cx.update_window(handle,|_,window,cx|{
        window.press("escape",cx);
        pane.read(cx).state.clone().update(cx,|editor,cx|{editor.set_value("answer = 42",window,cx);editor.set_selected_range(0..11,cx);editor.focus(window,cx);});
        window.press("ctrl-j",cx);
    })?;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        let index=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Selection completion menu missing"))?.content.items.iter().position(|item|item.label=="wrap").unwrap();
        for _ in 0..index{window.press("down",cx);}
        window.press("enter",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        ensure!(pane.read(cx).state.read(cx).value().as_str()=="answer = 42\n","Selected-code snippet did not replace the whole selection");
        window.press("escape",cx);window.remove_window();Ok(())
    })??;
    let db=Arc::new(Db::open(&output.join("session/snippets.sqlite"))?);
    for(name,width,height,theme,zoom)in[("regular",1280.,800.,"Vesper",1.),("compact",720.,480.,"Vesper",1.),("light",1280.,800.,"Solarized Light",1.),("zoom",1000.,750.,"Vesper",1.4)]{
        cx.update(|cx|{crate::theme::apply(theme,cx);crate::theme::set_zoom(zoom,cx);});
        let(handle,editor)=cx.update(|cx|gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:point(px(0.),px(0.)),size:size(px(width),px(height))})),show:false,focus:false,..Default::default()},cx,|window,cx|{
            let config=Config{onboarding_completed:true,workspace:output.join("session/solutions"),..Default::default()};
            let ws=cx.new(|cx|Workspace::from_storage(config,db.clone(),[None,None],window,cx));
            cx.new(|cx|SnippetEditor::new(ws.downgrade(),output.join(format!("session/snippets-{name}")),snippets::Settings::default(),Language::Cpp,window,cx))
        }))?;
        let handle=handle.into();cx.run_until_parked();cx.update_window(handle,|_,window,cx|window.render_frame(cx))?;
        if pixels{cx.capture_screenshot(handle)?.save(output.join(format!("snippet-editor-{name}.png")))?;}
        if name=="regular"{
            cx.update_window(handle,|_,window,cx|->Result<()>{
                window.click("new-snippet",cx);window.input("Fixture custom",cx);window.render_frame(cx);
                let before=editor.read(cx).source.read(cx).value().to_string();
                window.click("insert-Stop",cx);ensure!(editor.read(cx).source.read(cx).value().len()>before.len(),"Stop toolbar did not insert");
                window.render_frame(cx);window.click("save-snippet",cx);Ok(())
            })??;cx.run_until_parked();
            cx.update_window(handle,|_,window,cx|->Result<()>{
                ensure!(store::load(&editor.read(cx).dir).snippets.iter().any(|s|s.name=="Fixture custom"),"Manual snippet was not saved");
                let chip=window.find("drag-Cursor").bounds();let body=window.find("snippet-body-drop").bounds();
                window.drag(chip.center(),body.center(),cx);Ok(())
            })??;cx.run_until_parked();
            cx.update_window(handle,|_,window,cx|->Result<()>{
                ensure!(editor.read(cx).source.read(cx).value().matches("$0").count()==1,"Drag did not move the final cursor");
                window.render_frame(cx);window.click("snippet-ai",cx);Ok(())
            })??;cx.run_until_parked();cx.update_window(handle,|_,window,cx|window.render_frame(cx))?;
            if pixels{cx.capture_screenshot(handle)?.save(output.join("snippet-editor-ai.png"))?;}
            cx.update_window(handle,|_,window,cx|->Result<()>{
                editor.update(cx,|e,cx|{
                    let stage=e.dir.join("fixture-proposal");let mut proposal=e.user.clone();
                    proposal.push(Snippet{name:"Agent fixture".into(),prefixes:vec!["agentfixture".into()],body:"${1:answer} $0".into(),description:String::new(),scope:Some(Language::Cpp),template:false,origin:Origin::Ai});
                    store::save(&stage,&proposal).unwrap();e.ai.stage=Some(stage);e.ai.base=e.user.clone();e.ai.proposed=Some(proposal);cx.notify();
                });
                window.render_frame(cx);window.click("review-ai-cpp/Agent fixture",cx);window.render_frame(cx);
                window.click("insert-Choice",cx);window.render_frame(cx);window.click("save-snippet",cx);
                ensure!(!store::load(&editor.read(cx).dir).snippets.iter().any(|s|s.name=="Agent fixture"),"Review save changed the real library");
                window.render_frame(cx);window.click("apply-ai-snippets",cx);
                ensure!(store::load(&editor.read(cx).dir).snippets.iter().any(|s|s.name=="Agent fixture"&&s.body.contains("YES,NO")),"Reviewed edits were not applied");
                window.render_frame(cx);window.click("undo-ai-snippets",cx);
                ensure!(!store::load(&editor.read(cx).dir).snippets.iter().any(|s|s.name=="Agent fixture"),"Proposal Undo did not restore the library");
                Ok(())
            })??;cx.run_until_parked();
        }
        if name=="compact" {
            cx.update_window(handle,|_,window,cx|{window.render_frame(cx);window.click("snippet-ai",cx);})?;
            cx.run_until_parked();cx.update_window(handle,|_,window,cx|window.render_frame(cx))?;
            if pixels{cx.capture_screenshot(handle)?.save(output.join("snippet-editor-ai-compact.png"))?;}
        }
        cx.update_window(handle,|_,window,_|window.remove_window())?;cx.run_until_parked();
    }
    cx.update(|cx|{crate::theme::apply("Vesper",cx);crate::theme::set_zoom(1.,cx);});
    Ok(serde_json::json!({"fixture":"snippets","passed":true,"pixels":pixels,"checks":["prefix Tab expansion","typing mirrors","Tab and Shift+Tab","native cursor completions and Escape","merged language-server suggestions","Enter and Tab accept linked snippet placeholders","manual creation and save","drag cursor drop","compact light and zoom layouts","AI panel without agent startup","staged review edit, Apply and Undo"]}))
}
