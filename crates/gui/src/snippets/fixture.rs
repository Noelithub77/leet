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
    ensure!(cx.update(|cx|crate::theme::apply("Monokai Pro Spectrum",cx)),"Spectrum theme failed to load");
    cx.update(|cx|{super::bind_keys(cx);crate::snippets::expansion::bind_keys(cx);cx.bind_keys([KeyBinding::new("ctrl-j",crate::actions::InsertSnippet,None)]);});
    let(handle,pane)=cx.update(|cx|gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:point(px(0.),px(0.)),size:size(px(1000.),px(700.))})),show:false,focus:false,..Default::default()},cx,|window,cx|cx.new(|cx|{
        let state=cx.new(|cx|EditorState::new(window,cx).language("python"));
        let mut pane=crate::snippets::expansion::EditorPane::new(state.clone(),Language::Python,std::rc::Rc::new(practice::snippets::search::Library::new(vec![])),window,cx);
        state.update(cx,|editor,_|editor.lsp_mut().completion_provider=Some(std::rc::Rc::new(FixtureCompletions)));
        let mut library=vec![Snippet{name:"Fixture loop".into(),prefixes:vec!["loop".into(),"!loop".into()],body:"for ${1:i} in ${2:range(n)}:\n\tprint($1)\n$0".into(),description:"A fixture".into(),scope:Some(Language::Python),template:false,origin:Origin::User}];
        library.push(Snippet{name:"Wrap selection".into(),prefixes:vec!["wrap".into()],body:"${TM_SELECTED_TEXT}\n$0".into(),description:"Keep selected code".into(),scope:Some(Language::Python),template:false,origin:Origin::User});
        library.push(practice::snippets::builtin().into_iter().find(|s|s.scope==Some(Language::Python)&&s.prefix()=="bfs").unwrap());
        pane.library=std::rc::Rc::new(practice::snippets::search::Library::new(library));
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
        ensure!(window.find("source-completion-preview").visible(),"Full expansion preview hidden");
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
        ensure!(window.find("completion-preview-fade").bounds().size.width>px(300.),"Preview fade has no width");
        Ok(())
    })??;
    if pixels{
        cx.capture_screenshot(handle)?.save(output.join("snippet-bfs-help.png"))?;
        cx.update(|cx|{crate::theme::apply("Vesper",cx);});
        cx.update_window(handle,|_,window,cx|{window.render_frame(cx);})?;
        cx.capture_screenshot(handle)?.save(output.join("snippet-bfs-help-vesper.png"))?;
    }
    cx.update_window(handle,|_,window,cx|{
        window.press("escape",cx);
        pane.read(cx).state.clone().update(cx,|editor,cx|{editor.set_value("answer = 42",window,cx);editor.set_selected_range(0..11,cx);editor.focus(window,cx);});
        window.press("ctrl-j",cx);
    })?;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        let menu=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Selection completion menu missing"))?;
        let index=menu.content.items.iter().position(|item|item.label=="wrap").unwrap();
        let selected=menu.selected;
        for _ in 0..selected{window.press("up",cx);}
        for _ in 0..index{window.press("down",cx);}
        window.press("enter",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        ensure!(pane.read(cx).state.read(cx).value().as_str()=="answer = 42\n","Selected-code snippet did not replace the whole selection");
        window.press("escape",cx);window.remove_window();Ok(())
    })??;
    let db=Arc::new(Db::open(&output.join("session/snippets.sqlite"))?);
    for(name,width,height,theme,zoom)in[("regular",1280.,800.,"Vesper",1.),("spectrum",1280.,800.,"Monokai Pro Spectrum",1.),("compact",720.,480.,"Vesper",1.),("light",1280.,800.,"Solarized Light",1.),("zoom",1000.,750.,"Vesper",1.4)]{
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
    workspace_tab(cx,output,pixels)?;
    delayed_autocomplete(cx)?;
    Ok(serde_json::json!({"fixture":"snippets","passed":true,"pixels":pixels,"checks":["prefix Tab expansion","typing mirrors","Tab and Shift+Tab","native cursor completions and Escape","merged language-server suggestions","Enter and Tab accept linked snippet placeholders","manual creation and save","drag cursor drop","compact light and zoom layouts","AI panel without agent startup","staged review edit, Apply and Undo","workspace tab and Explorer","field titles","Ctrl+W close and invalid-save guard","immediate local fuzzy results with delayed LSP","late merge preserves selection","Escape cancels late LSP","safe cached server results while extending and deleting a word","external snippet hot reload and invalid-file recovery"]}))
}

fn workspace_tab(cx:&mut HeadlessAppContext,output:&Path,pixels:bool)->Result<()> {
    use crate::workspace::{Center,Workspace};
    let config=Config{onboarding_completed:true,workspace:output.join("session/tab-solutions"),..Config::default()};
    let db=Arc::new(Db::open(&output.join("session/snippet-tab.sqlite"))?);
    cx.update(|cx|{
        let bindings=cx.key_bindings().borrow().bindings().cloned().collect();
        cx.set_global(crate::actions::ComponentBindings(bindings));
        crate::actions::reload_keys(&config,cx);
    });
    let(handle,workspace)=cx.update(|cx|gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:point(px(0.),px(0.)),size:size(px(1280.),px(800.))})),show:false,focus:false,..Default::default()},cx,|window,cx|cx.new(|cx|Workspace::from_storage(config,db,[None,None],window,cx))))?;
    let handle=handle.into();cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|{workspace.update(cx,|view,cx|view.focus_nav(crate::workspace::Focus::Home,window,cx));window.render_frame(cx);window.press("ctrl-shift-s",cx);})?;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        window.render_frame(cx);
        ensure!(workspace.read(cx).center==Center::Snippets,"Snippet shortcut did not open the editor");
        ensure!(window.find("snippets-tab").visible(),"Snippet workspace tab missing");
        ensure!(workspace.read(cx).left,"Explorer hidden in snippet tab");
        for id in ["snippet-name-label","snippet-prefix-label","snippet-description-label"] {ensure!(window.find(id).visible(),"Snippet field title missing: {id}");}
        window.click("toggle-left-panel",cx);
        Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        ensure!(!workspace.read(cx).left,"Explorer did not hide");
        window.render_frame(cx);window.click("toggle-left-panel",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,_,cx|->Result<()>{
        ensure!(workspace.read(cx).left&&workspace.read(cx).center==Center::Snippets,"Explorer toggle left snippet tab");Ok(())
    })??;
    cx.update_window(handle,|_,window,cx|window.render_frame(cx))?;
    if pixels{cx.capture_screenshot(handle)?.save(output.join("snippet-workspace-tab.png"))?;}
    cx.update_window(handle,|_,window,cx|window.hover("snippet-prefix-label",cx))?;
    cx.advance_clock(std::time::Duration::from_millis(700));cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|window.render_frame(cx))?;
    if pixels{cx.capture_screenshot(handle)?.save(output.join("snippet-prefix-help.png"))?;}
    cx.update_window(handle,|_,window,cx|{window.press("ctrl-h",cx);})?;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        window.render_frame(cx);ensure!(workspace.read(cx).center==Center::Home&&workspace.read(cx).snippet_editor.is_some(),"Switching tabs closed snippets");
        window.click("snippets-tab",cx);Ok(())
    })??;cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->Result<()>{
        ensure!(workspace.read(cx).center==Center::Snippets,"Snippet tab click failed");
        let editor=workspace.read(cx).snippet_editor.clone().unwrap();
        editor.read(cx).source.clone().update(cx,|source,cx|{source.set_value("${1:unfinished",window,cx);source.focus(window,cx);});
        window.press("ctrl-w",cx);
        ensure!(workspace.read(cx).snippet_editor.is_some(),"Closing discarded invalid edits");
        editor.read(cx).source.clone().update(cx,|source,cx|source.set_value("$0",window,cx));
        window.press("ctrl-w",cx);
        ensure!(workspace.read(cx).snippet_editor.is_none()&&workspace.read(cx).center==Center::Home,"Ctrl+W did not close snippets");
        Ok(())
    })??;cx.run_until_parked();
    let (directory, watcher)=cx.update(|cx|workspace.update(cx,|ws,cx|(ws.snippet_dir.clone(),ws.watch_snippets(cx))));
    cx.run_until_parked();
    let mut user=store::load(&directory).snippets;
    user.push(Snippet{name:"Hot reload fixture".into(),prefixes:vec!["hotfixture".into()],body:"${1:fresh}$0".into(),description:"Live external edit".into(),scope:Some(Language::Python),template:false,origin:Origin::User});
    store::save(&directory,&user)?;
    cx.advance_clock(std::time::Duration::from_millis(850));cx.run_until_parked();
    cx.update_window(handle,|_,_,cx|->Result<()>{
        ensure!(workspace.read(cx).editor_pane.read(cx).library.iter().any(|snippet|snippet.prefix()=="hotfixture"),"External snippet edit did not refresh the retained index");Ok(())
    })??;
    std::fs::write(directory.join("python.json"),"invalid temporary edit")?;
    cx.advance_clock(std::time::Duration::from_millis(850));cx.run_until_parked();
    cx.update_window(handle,|_,_,cx|->Result<()>{
        ensure!(workspace.read(cx).editor_pane.read(cx).library.iter().any(|snippet|snippet.prefix()=="hotfixture"),"Invalid library erased the last usable index");Ok(())
    })??;
    user.last_mut().unwrap().prefixes=vec!["hotfixed".into()];
    store::save(&directory,&user)?;
    cx.advance_clock(std::time::Duration::from_millis(850));cx.run_until_parked();
    cx.update_window(handle,|_,_,cx|->Result<()>{
        ensure!(workspace.read(cx).editor_pane.read(cx).library.iter().any(|snippet|snippet.prefix()=="hotfixed"),"Repaired library did not replace the retained index");Ok(())
    })??;
    drop(watcher);
    cx.update_window(handle,|_,window,_|window.remove_window())?;cx.run_until_parked();Ok(())
}

struct DelayedCompletions;
impl CompletionProvider for DelayedCompletions {
    fn is_completion_trigger(&self, _: usize, _: &str, _: &mut App) -> bool { true }
    fn completions(&self, _: &ropey::Rope, _: usize, _: lsp_types::CompletionContext, _: &mut Window, cx: &mut App) -> Task<anyhow::Result<lsp_types::CompletionResponse>> {
        cx.spawn(async move |cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            Ok(lsp_types::CompletionResponse::Array(vec![
                lsp_types::CompletionItem { label: "delayed_variable".into(), ..Default::default() },
                lsp_types::CompletionItem { label: "reachable".into(),kind:Some(lsp_types::CompletionItemKind::VARIABLE), ..Default::default() },
                lsp_types::CompletionItem { label: "break".into(),kind:Some(lsp_types::CompletionItemKind::KEYWORD), ..Default::default() }
            ]))
        })
    }
}
fn delayed_autocomplete(cx: &mut HeadlessAppContext) -> Result<()> {
    let (handle, pane) = cx.update(|cx| gpui_kit::open_window(WindowOptions { show:false, focus:false, ..Default::default() }, cx, |window,cx| cx.new(|cx| {
        let state = cx.new(|cx| EditorState::new(window,cx).language("python"));
        let library = std::rc::Rc::new(practice::snippets::search::Library::new(practice::snippets::builtin()));
        let pane = crate::snippets::expansion::EditorPane::new(state.clone(),Language::Python,library,window,cx);
        state.update(cx, |editor,cx| { editor.lsp_mut().completion_provider=Some(std::rc::Rc::new(DelayedCompletions)); editor.focus(window,cx); });
        pane
    })))?;
    let handle=handle.into(); cx.run_until_parked();
    for query in ["brfs", "reachable", "bfs"] {
        cx.update_window(handle, |_,window,cx| {
            window.render_frame(cx);
            pane.read(cx).state.clone().update(cx, |editor,cx| { editor.set_value("",window,cx); editor.focus(window,cx); });
            window.input(query,cx);
        })?;
        cx.run_until_parked();
        cx.update_window(handle, |_,window,cx| -> Result<()> {
            window.render_frame(cx);
            let menu=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Local results waited for LSP: {query}"))?;
            let expected=if query=="reachable" {"dfs"} else {"bfs"};
            ensure!(menu.content.items.iter().any(|item|item.label==expected),"Fuzzy name/description missing: {query}");
            ensure!(!menu.content.items.iter().any(|item|item.label=="delayed_variable"),"Delayed fixture returned before its timer");
            window.press("down",cx);
            Ok(())
        })??;
        let selected=cx.update(|cx| { let menu=pane.read(cx).menu.as_ref().unwrap(); menu.content.items[menu.selected].clone() });
        cx.advance_clock(std::time::Duration::from_millis(550)); cx.run_until_parked();
        cx.update_window(handle, |_,_,cx| -> Result<()> {
            let menu=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Late merge hid menu"))?;
            ensure!(menu.content.items.iter().any(|item|item.label=="delayed_variable"),"Late LSP was never merged");
            ensure!(menu.content.items[menu.selected]==selected,"Late merge changed keyboard selection");
            if query=="reachable" { ensure!(menu.content.items[0].label=="reachable","Exact symbol ranked behind a weak snippet description match"); }
            let first_snippet=menu.content.items.iter().position(|item|item.kind==Some(lsp_types::CompletionItemKind::SNIPPET)).unwrap();
            ensure!(menu.content.items[..first_snippet].iter().any(|item|item.kind==Some(lsp_types::CompletionItemKind::KEYWORD)),"Keyword ranked behind snippets: {query}");
            ensure!(menu.content.items[first_snippet..].iter().all(|item|item.kind==Some(lsp_types::CompletionItemKind::SNIPPET)),"Language suggestion ranked behind snippets: {query}");
            Ok(())
        })??;
    }
    cx.update_window(handle, |_,window,cx| {
        pane.read(cx).state.clone().update(cx, |editor,cx| { editor.set_value("",window,cx); editor.focus(window,cx); });
        window.input("delayed",cx);
    })?; cx.run_until_parked();
    cx.advance_clock(std::time::Duration::from_millis(550)); cx.run_until_parked();
    cx.update_window(handle, |_,window,cx| { window.input("_vx",cx); })?; cx.run_until_parked();
    cx.update_window(handle, |_,window,cx| { window.render_frame(cx); window.press("backspace",cx); })?; cx.run_until_parked();
    cx.update_window(handle, |_,window,cx| -> Result<()> {
        window.render_frame(cx);
        let menu=pane.read(cx).menu.as_ref().ok_or_else(||anyhow::anyhow!("Cached server rows waited for another request"))?;
        ensure!(menu.content.items.iter().any(|item|item.label=="delayed_variable"),"Cached server result missing");
        window.press("enter",cx); Ok(())
    })??; cx.run_until_parked();
    cx.update_window(handle, |_,_,cx| -> Result<()> {
        ensure!(pane.read(cx).state.read(cx).value().as_str()=="delayed_variable","Cached acceptance duplicated the word"); Ok(())
    })??;
    cx.update_window(handle, |_,window,cx| { window.input("x",cx); })?; cx.run_until_parked();
    cx.update_window(handle, |_,window,cx| { window.render_frame(cx); window.press("escape",cx); })?;
    cx.advance_clock(std::time::Duration::from_millis(550)); cx.run_until_parked();
    cx.update_window(handle, |_,window,cx| -> Result<()> {
        ensure!(pane.read(cx).menu.is_none(),"Escaped menu reopened after late LSP");
        window.remove_window(); Ok(())
    })??; cx.run_until_parked(); Ok(())
}
