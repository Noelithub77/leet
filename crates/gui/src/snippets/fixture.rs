//! Real input against production snippet views; no agents or editor scans run.
use super::*;
use anyhow::{Result,ensure};
use std::{path::Path,sync::Arc};
use gpui_kit::test::TestWindowExt as _;
use practice::{config::Config,db::Db};

pub(crate) fn native(cx:&mut HeadlessAppContext,output:&Path,pixels:bool)->Result<serde_json::Value>{
    cx.update(|cx|{super::bind_keys(cx);crate::snippets::expansion::bind_keys(cx);cx.bind_keys([KeyBinding::new("ctrl-j",crate::actions::InsertSnippet,None)]);});
    let(handle,pane)=cx.update(|cx|gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:point(px(0.),px(0.)),size:size(px(1000.),px(700.))})),show:false,focus:false,..Default::default()},cx,|window,cx|cx.new(|cx|{
        let state=cx.new(|cx|EditorState::new(window,cx).language("python"));
        let mut pane=crate::snippets::expansion::EditorPane::new(state.clone(),Language::Python,window,cx);
        pane.library=vec![Snippet{name:"Fixture loop".into(),prefixes:vec!["loop".into(),"!loop".into()],body:"for ${1:i} in ${2:range(n)}:\n\tprint($1)\n$0".into(),description:"A fixture".into(),scope:Some(Language::Python),template:false,origin:Origin::User}];
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
        window.press("escape",cx);window.press("ctrl-j",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{window.render_frame(cx);ensure!(window.find("snippet-picker").visible(),"Snippet picker hidden");window.input("fixture",cx);Ok(())})??;
    cx.run_until_parked();cx.update_window(handle,|_,window,cx|window.render_frame(cx))?;
    if pixels{cx.capture_screenshot(handle)?.save(output.join("snippet-picker.png"))?;}
    cx.update_window(handle,|_,window,cx|{window.press("escape",cx);window.remove_window();})?;
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
    Ok(serde_json::json!({"fixture":"snippets","passed":true,"pixels":pixels,"checks":["prefix Tab expansion","typing mirrors","Tab and Shift+Tab","picker search and Escape","manual creation and save","drag cursor drop","compact light and zoom layouts","AI panel without agent startup","staged review edit, Apply and Undo"]}))
}
