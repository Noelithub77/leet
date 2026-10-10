//! Full-width snippet library and manual creator. AI works on a reviewable copy.
use std::path::PathBuf;
use gpui_kit::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, Input, InputState, InputEvent};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use practice::{language::Language, snippets::{self, Snippet, Origin, store, body}};
use crate::workspace::{Workspace, Center};

gpui_kit::actions!(snippet_editor, [SaveSnippet]);
pub fn bind_keys(cx: &mut App) { cx.bind_keys([KeyBinding::new("ctrl-enter", SaveSnippet, Some("SnippetEditor")),KeyBinding::new("ctrl-enter", SaveSnippet, Some("SnippetEditor > Input"))]); }
#[derive(Clone)]
struct StopChip(String);
impl Render for StopChip { fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement { div().px_3().py_2().rounded_md().bg(cx.theme().primary).text_color(cx.theme().primary_foreground).child(self.0.clone()) } }

pub struct SnippetEditor {
    pub(super) workspace: WeakEntity<Workspace>,
    pub(super) dir: PathBuf,
    pub(super) user: Vec<Snippet>,
    library: std::rc::Rc<snippets::search::Library>,
    searcher: practice::search::Searcher,
    settings: snippets::Settings,
    pub(super) language: Language,
    selected: Option<String>,
    pub(super) original: Option<Snippet>,
    name: Entity<InputState>,
    prefixes: Entity<InputState>,
    description: Entity<InputState>,
    search: Entity<InputState>,
    pub(super) source: Entity<EditorState>,
    preview: Entity<EditorState>,
    template: bool,
    pub(super) error: Option<String>,
    status: String,
    scanning: bool,
    pub(super) ai: super::ai::State,
    _subscriptions: Vec<Subscription>,
}
impl Workspace {
    pub fn starting_snippet(&self, language: Language, cx: &App) -> Option<String> {
        let name=self.config.snippets.templates.get(language.id())?;
        let snippet=self.editor_pane.read(cx).library.iter().find(|s|s.name==*name&&s.applies_to(language)&&s.template)?;
        Some(snippet.body.clone())
    }
    pub fn reload_snippets(&mut self, cx: &mut Context<Self>) {
        let initial_load = self.snippet_epoch == 0;
        self.snippet_epoch = self.snippet_epoch.wrapping_add(1);
        let epoch = self.snippet_epoch;
        let directory = self.snippet_dir.clone(); let settings = self.config.snippets.clone();
        cx.spawn(async move |this, cx| {
            let library = cx.background_spawn(async move {
                let loaded = store::load(&directory);
                if !initial_load && !loaded.errors.is_empty() { return None; }
                Some(snippets::search::Library::new(store::effective(&loaded.snippets, &snippets::builtin(), &settings)))
            }).await;
            if let Some(library) = library {
                let _ = this.update(cx, |ws, cx| {
                    if ws.snippet_epoch != epoch { return; }
                    ws.snippet_library = std::rc::Rc::new(library);
                    ws.apply_snippet_library(cx);
                });
            }
        }).detach();
    }
    pub(crate) fn watch_snippets(&self, cx: &mut Context<Self>) -> Task<()> {
        let directory = self.snippet_dir.clone();
        cx.spawn(async move |this, cx| {
            let mut revision = cx.background_spawn({ let directory=directory.clone(); async move { store::revision(&directory) } }).await;
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(800)).await;
                let current = cx.background_spawn({ let directory=directory.clone(); async move { store::revision(&directory) } }).await;
                if current != revision {
                    revision = current;
                    if this.update(cx, |ws,cx| ws.reload_snippets(cx)).is_err() { break; }
                } else if this.upgrade().is_none() { break; }
            }
        })
    }
    pub(crate) fn apply_snippet_library(&mut self, cx: &mut Context<Self>) {
        let library = self.snippet_library.clone();
        let tab_expand = self.config.snippets.tab_expand;
        let directory=self.config.workspace.clone();
        self.editor_pane.update(cx, |pane,cx| {pane.library=library;pane.tab_expand=tab_expand;pane.workspace=Some(directory);cx.notify();});
        self.update_tab_snippets(cx);
    }
    pub fn open_snippet_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_now(cx);
        if let Some(editor)=&self.snippet_editor { editor.update(cx,|e,cx|{if !e.dirty(cx){e.refresh(window,cx);}}); }
        else {
            let workspace=cx.weak_entity();let dir=self.snippet_dir.clone();let settings=self.config.snippets.clone();let language=self.config.preferred_language;
            self.snippet_editor=Some(cx.new(|cx|SnippetEditor::new(workspace,dir,settings,language,window,cx)));
        }
        self.center=Center::Snippets;
        if let Some(editor)=&self.snippet_editor { editor.update(cx,|e,cx|e.search.update(cx,|s,cx|s.focus(window,cx))); }
        cx.notify();
    }
    pub fn close_snippet_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = self.snippet_editor.clone() {
            if !editor.update(cx, |editor, cx| editor.save(window, cx)) { return; }
        }
        self.snippet_editor = None;
        if self.center == Center::Snippets { self.back_to_editor(window, cx); }
        cx.notify();
    }
}
impl SnippetEditor {
    pub fn new(workspace: WeakEntity<Workspace>, dir:PathBuf, settings:snippets::Settings, language:Language, window:&mut Window,cx:&mut Context<Self>)->Self {
        let name=cx.new(|cx|InputState::new(window,cx).placeholder("Name"));
        let prefixes=cx.new(|cx|InputState::new(window,cx).placeholder("Prefix, aliases"));
        let description=cx.new(|cx|InputState::new(window,cx).placeholder("Description (optional)"));
        let search=cx.new(|cx|InputState::new(window,cx).placeholder("Search snippets…"));
        let source=cx.new(|cx|EditorState::new(window,cx).language(language.id()).line_number(true).soft_wrap(false));
        let preview=cx.new(|cx|EditorState::new(window,cx).language(language.id()).line_number(true));
        let changed=cx.subscribe_in(&source,window,|this,_,event,window,cx|if matches!(event,InputEvent::Change){this.update_preview(window,cx);});
        let mut subscriptions=vec![changed];
        for input in [&name,&prefixes,&description,&search] {subscriptions.push(cx.subscribe(input,|_,_,_:&InputEvent,cx|cx.notify()));}
        let ai=super::ai::State::new(window,cx);
        let mut this=Self{workspace,dir,user:Vec::new(),library:std::rc::Rc::new(snippets::search::Library::new(vec![])),searcher:practice::search::Searcher::default(),settings,language,selected:None,original:None,name,prefixes,description,search,source,preview,template:false,error:None,status:String::new(),scanning:false,ai,_subscriptions:subscriptions};
        this.refresh(window,cx);this
    }
    pub(super) fn refresh(&mut self, window:&mut Window,cx:&mut Context<Self>) {
        let loaded=store::load(&self.dir);
        self.error=loaded.errors.first().map(|(p,e)|format!("{}: {e}",p.display()));
        self.user=loaded.snippets;
        self.library=std::rc::Rc::new(snippets::search::Library::new(store::effective(&self.user,&snippets::builtin(),&self.settings)));
        if self.original.is_none() {if let Some(s)=self.library.iter().find(|s|s.applies_to(self.language)).cloned(){self.load(s,window,cx);}}
        cx.notify();
    }
    fn load(&mut self,s:Snippet,window:&mut Window,cx:&mut Context<Self>){
        self.selected=Some(s.key());self.template=s.template;
        self.name.update(cx,|e,cx|e.set_value(s.name.clone(),window,cx));
        self.prefixes.update(cx,|e,cx|e.set_value(s.prefixes.join(", "),window,cx));
        self.description.update(cx,|e,cx|e.set_value(s.description.clone(),window,cx));
        self.source.update(cx,|e,cx|{e.set_highlighter(self.language.id(),cx);e.set_value(s.body.clone(),window,cx);});
        self.original=Some(s);self.update_preview(window,cx);
    }
    pub(super) fn draft(&self,cx:&App)->Snippet {
        Snippet{name:self.name.read(cx).value().trim().into(),prefixes:self.prefixes.read(cx).value().split(',').map(str::trim).filter(|s|!s.is_empty()).map(str::to_owned).collect(),description:self.description.read(cx).value().trim().into(),body:self.source.read(cx).value().into(),scope:Some(self.language),template:self.template,origin:self.original.as_ref().map_or(Origin::User,|s|if s.origin==Origin::Builtin{Origin::User}else{s.origin})}
    }
    fn dirty(&self,cx:&App)->bool {let s=self.draft(cx);self.original.as_ref().is_none_or(|o|s.name!=o.name||s.prefixes!=o.prefixes||s.description!=o.description||s.body!=o.body||o.scope.is_some()&&s.scope!=o.scope||s.template!=o.template)}
    fn update_preview(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let text=body::preview(self.source.read(cx).value().as_str()).text;
        self.preview.update(cx,|e,cx|{e.set_highlighter(self.language.id(),cx);e.set_value(text,window,cx);});cx.notify();
    }
    pub(super) fn save(&mut self,window:&mut Window,cx:&mut Context<Self>)->bool {
        if !self.dirty(cx){return true;}
        let draft=self.draft(cx);
        if let Some(proposed)=&self.ai.proposed && let Some(original)=&self.original && proposed.contains(original) {
            let others:Vec<_>=proposed.iter().filter(|s|s.key()!=original.key()).cloned().collect();
            if let Err(e)=store::check(&draft,&others){self.error=Some(e);cx.notify();return false;}
            let mut revised=others;revised.push(draft.clone());
            let Some(stage)=&self.ai.stage else{return false;};
            if let Err(e)=store::commit(stage,proposed,&revised){self.error=Some(e.to_string());cx.notify();return false;}
            self.ai.proposed=Some(revised);self.selected=Some(draft.key());self.original=Some(draft);self.status="Review saved · apply when ready".into();cx.notify();return true;
        }
        let others:Vec<_>=self.library.iter().filter(|s|Some(&s.key())!=self.selected.as_ref()).cloned().collect();
        if let Err(e)=store::check(&draft,&others){self.error=Some(e);cx.notify();return false;}
        let mut user=self.user.clone();user.retain(|s|Some(&s.key())!=self.selected.as_ref());user.push(draft.clone());
        if let Err(e)=store::commit(&self.dir,&self.user,&user){self.error=Some(e.to_string());cx.notify();return false;}
        self.user=user;self.original=Some(draft.clone());self.selected=Some(draft.key());self.error=None;self.status="Saved".into();
        self.library=std::rc::Rc::new(snippets::search::Library::new(store::effective(&self.user,&snippets::builtin(),&self.settings)));self.notify_workspace(window,cx);cx.notify();true
    }
    pub(super) fn notify_workspace(&self,window:&mut Window,cx:&mut Context<Self>){
        let workspace=self.workspace.clone();window.defer(cx,move|_,cx|{let _=workspace.update(cx,|ws,cx|ws.reload_snippets(cx));});
    }
    fn create(&mut self,window:&mut Window,cx:&mut Context<Self>){
        if !self.save(window,cx){return;}
        let s=Snippet{name:store::unique_name("New snippet",Some(self.language),&self.library),prefixes:vec!["snippet".into()],body:"$0".into(),description:String::new(),scope:Some(self.language),template:false,origin:Origin::User};
        self.load(s,window,cx);self.selected=None;self.original=None;self.name.update(cx,|e,cx|{e.select_all(window,cx);e.focus(window,cx);});cx.notify();
    }
    fn remove(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let Some(key)=self.selected.clone()else{return;};
        if self.original.as_ref().is_some_and(|s|s.origin==Origin::Builtin){
            self.settings.hidden.push(key);self.persist_settings(window,cx);
        }else{
            let mut user=self.user.clone();user.retain(|s|s.key()!=key);
            if let Err(e)=store::commit(&self.dir,&self.user,&user){self.error=Some(e.to_string());cx.notify();return;}
            self.ai.undo=Some((self.user.clone(),user.clone()));self.user=user;
        }
        self.original=None;self.selected=None;self.refresh(window,cx);self.notify_workspace(window,cx);
    }
    fn persist_settings(&self,window:&mut Window,cx:&mut Context<Self>){
        let workspace=self.workspace.clone();let settings=self.settings.clone();
        window.defer(cx,move|window,cx|{let _=workspace.update(cx,|ws,cx|{ws.config.snippets=settings;ws.save_config(window,cx);ws.reload_snippets(cx);});});
    }
    pub(super) fn stage(&mut self,s:Snippet,window:&mut Window,cx:&mut Context<Self>){self.load(s,window,cx);cx.notify();}
    fn import(&mut self,window:&mut Window,cx:&mut Context<Self>){
        if self.scanning || !self.save(window,cx){return;}self.scanning=true;
        let dir=self.dir.clone();let expected=self.user.clone();
        cx.spawn_in(window,async move|this,cx|{
            let result=cx.background_spawn(async move{
                let found=snippets::import::scan(&snippets::import::Roots::detect());
                let skipped:usize=found.iter().map(|f|f.skipped).sum();
                let incoming=found.into_iter().flat_map(|f|f.snippets).collect();let mut user=expected.clone();let report=snippets::import::merge(&mut user,incoming);
                store::commit(&dir,&expected,&user)?;anyhow::Ok((report.added,skipped))
            }).await;
            let _=this.update_in(cx,|this,window,cx|{this.scanning=false;match result{Ok((added,skipped))=>{this.status=format!("Imported {added} · skipped {skipped}");this.refresh(window,cx);this.notify_workspace(window,cx);},Err(e)=>this.error=Some(e.to_string())}cx.notify();});
        }).detach();cx.notify();
    }
    fn insert_chip(&mut self,syntax:String,point:Option<Point<Pixels>>,window:&mut Window,cx:&mut Context<Self>){
        self.source.update(cx,|e,cx|{
            if let Some(point)=point && let Some(ix)=e.character_index_for_point(point,window,cx){
                let text=e.value();let mut units=0;let mut offset=text.len();for(i,c)in text.char_indices(){if units>=ix{offset=i;break;}units+=c.len_utf16();}e.set_selected_range(offset..offset,cx);
            }
            if syntax == "$0" {
                let text=e.value();let position=e.selected_range().start;
                let (text,position)=body::place_cursor(&text,position);
                e.replace_all(text,window,cx);e.set_selected_range(position+2..position+2,cx);
            } else { e.replace(syntax,window,cx); }
            e.focus(window,cx);
        });self.update_preview(window,cx);
    }
}
impl Render for SnippetEditor {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let theme=cx.theme().clone();let query=self.search.read(cx).value().to_lowercase();
        let items:Vec<_>=self.library.rank(&mut self.searcher,&query,self.language,self.library.len()).into_iter().map(|row|self.library.row(row).0.clone()).collect();
        let expansion=body::preview(self.source.read(cx).value().as_str());let next=expansion.stops.iter().map(|s|s.index).max().unwrap_or(0).saturating_add(1);
        let validation=body::validate(self.source.read(cx).value().as_str()).err();
        v_flex().id("snippet-editor").test_support().size_full().key_context("SnippetEditor").p_4().gap_3()
            .on_action(cx.listener(|this,_:&SaveSnippet,w,cx|{this.save(w,cx);}))
            .child(h_flex().gap_3().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).flex_1().child("Snippet details"))
                .child(Button::new("snippet-ai").ghost().small().icon(IconName::Sparkles).tooltip("Create or edit with AI").on_click(cx.listener(|this,_,_,cx|{this.ai.open=!this.ai.open;cx.notify();}))))
            .child(h_flex().gap_1().children(Language::ALL.into_iter().map(|language|{
                crate::theme::selected_choice(Button::new(SharedString::from(format!("snippet-language-{}",language.id()))).ghost().small(),self.language==language,cx).icon(Icon::default().path(format!("languages/{}.svg",language.id())).small()).label(language.label()).on_click(cx.listener(move|this,_,w,cx|{if this.save(w,cx){this.language=language;this.original=None;this.selected=None;this.refresh(w,cx);}}))
            })))
            .child(h_flex().flex_1().min_h_0().items_stretch().gap_3()
                .child(v_flex().w(px(if window.viewport_size().width < px(900.) {170.}else{230.})).min_h_0().gap_2()
                    .child(div().text_xs().text_color(theme.muted_foreground).child("Library"))
                    .child(h_flex().gap_1().child(div().flex_1().child(Input::new(&self.search).small()))
                        .child(Button::new("new-snippet").ghost().small().icon(IconName::Plus).tooltip("New snippet").on_click(cx.listener(|this,_,w,cx|this.create(w,cx)))))
                    .child(uniform_list("snippet-library",items.len(),cx.processor(move|this,range:std::ops::Range<usize>,_,cx|{
                        range.filter_map(|ix|items.get(ix)).map(|s|{let s=s.clone();let key=s.key();
                            crate::theme::selected_choice(Button::new(SharedString::from(format!("snippet-row-{key}"))).ghost(),this.selected.as_ref()==Some(&key),cx).w_full().label(s.name.clone()).tooltip(format!("{} · {}",s.prefix(),s.origin.label())).on_click(cx.listener(move|this,_,w,cx|{if this.save(w,cx){this.load(s.clone(),w,cx);}}))
                        }).collect::<Vec<_>>()
                    })).flex_1().min_h_0())
                    .child(Button::new("import-snippets").outline().small().icon(IconName::Download).label(if self.scanning{"Scanning…"}else{"Import / rescan"}).disabled(self.scanning).tooltip("Copy snippets from VS Code, Neovim and Sublime Text").on_click(cx.listener(|this,_,w,cx|this.import(w,cx))))
                    .child(Button::new("snippet-builtins").ghost().small().label(if self.settings.builtin{"Practice library ✓"}else{"Practice library"}).on_click(cx.listener(|this,_,w,cx|{this.settings.builtin=!this.settings.builtin;this.settings.hidden.clear();this.persist_settings(w,cx);this.refresh(w,cx);}))))
                .child(v_flex().flex_1().min_w_0().min_h_0().gap_2()
                    .child(h_flex().gap_2().items_end()
                        .child(v_flex().flex_1().min_w_0().gap_1().child(div().id("snippet-name-label").test_support().text_xs().text_color(theme.muted_foreground).child("Name")
                            .tooltip(|window,cx|crate::view::help_tooltip("Name", "A descriptive name shown in the library and autocomplete.", window, cx))).child(Input::new(&self.name).small()))
                        .child(v_flex().flex_1().min_w_0().gap_1().child(div().id("snippet-prefix-label").test_support().text_xs().text_color(theme.muted_foreground).child("Prefixes")
                            .tooltip(|window,cx|crate::view::help_tooltip("Prefixes", "Type a prefix and press Tab to insert. Separate aliases with commas, such as bfs, breadthfirst.", window, cx))).child(Input::new(&self.prefixes).small()))
                        .child(Button::new("save-snippet").primary().small().icon(IconName::Check).tooltip("Save · Ctrl+Enter").disabled(!self.dirty(cx)).on_click(cx.listener(|this,_,w,cx|{this.save(w,cx);})))
                        .child(Button::new("remove-snippet").ghost().small().icon(IconName::Trash).tooltip("Remove snippet").on_click(cx.listener(|this,_,w,cx|this.remove(w,cx)))))
                    .child(v_flex().gap_1().child(div().id("snippet-description-label").test_support().text_xs().text_color(theme.muted_foreground).child("Description")
                        .tooltip(|window,cx|crate::view::help_tooltip("Description", "One short line explaining what the snippet does. Shown beside its code preview in autocomplete.", window, cx))).child(Input::new(&self.description).small()))
                    .child(h_flex().gap_2().child(Button::new("snippet-template").ghost().small().label(if self.template{"File template ✓"}else{"File template"}).tooltip("Use this snippet as starter code for a new solution. Mark it as a template, then choose Use for new stdin solutions.").on_click(cx.listener(|this,_,_,cx|{this.template=!this.template;cx.notify();})))
                        .when(self.template,|row|row.child(Button::new("snippet-default-template").ghost().small().icon(IconName::File).tooltip("Use for new stdin solutions").on_click(cx.listener(|this,_,w,cx|{if this.save(w,cx){let name=this.name.read(cx).value().to_string();this.settings.templates.insert(this.language.id().into(),name);this.persist_settings(w,cx);}})))))
                    .child(h_flex().gap_2().children([(format!("${{{next}:value}}"),"Stop"),("$0".into(),"Cursor"),(format!("${{{next}|YES,NO|}}"),"Choice")].into_iter().map(|(syntax,label)|{
                        let help=match label { "Stop"=>"Click or drag to add an editable placeholder. Tab moves to the next stop; repeated stop numbers mirror your edits.", "Cursor"=>"Click or drag to set the final cursor position after all placeholders are filled. Use one $0 in the body.", _=>"Click or drag to add a placeholder with options. Choose a value after insertion, then press Tab to continue." };
                        let drag=StopChip(syntax.clone());div().id(SharedString::from(format!("drag-{label}"))).test_support()
                            .tooltip(move|window,cx|crate::view::help_tooltip(label,help,window,cx))
                            .on_drag(drag,|chip,_,_,cx|cx.new(|_|chip.clone())).child(Button::new(SharedString::from(format!("insert-{label}"))).outline().small().label(label).on_click(cx.listener(move|this,_,w,cx|this.insert_chip(syntax.clone(),None,w,cx))))
                    })))
                    .child(h_flex().flex_1().min_h_0().items_stretch().gap_3()
                        .child(v_flex().flex_1().min_w_0().gap_1().child(div().id("snippet-body-label").text_xs().text_color(theme.muted_foreground).child("Snippet code").tooltip(|window,cx|crate::view::help_tooltip("Snippet code", "Write code with ${1:placeholder} for editable stops, repeated $1 for linked values, and $0 for the final cursor.", window, cx)))
                            .child(div().id("snippet-body-drop").test_support().flex_1().min_h_0().on_drop(cx.listener(|this,chip:&StopChip,w,cx|this.insert_chip(chip.0.clone(),Some(w.mouse_position()),w,cx))).child(Editor::new(&self.source).h_full())))
                        .when(!self.ai.open || window.viewport_size().width >= px(1000.), |row|row.child(v_flex().flex_1().min_w_0().gap_1().child(div().id("snippet-preview-label").text_xs().text_color(theme.muted_foreground).child("Expanded preview").tooltip(|window,cx|crate::view::help_tooltip("Expanded preview", "See the inserted code with placeholder defaults filled in. Click a numbered stop below to locate it in the preview.", window, cx)))
                            .child(div().flex_1().min_h_0().child(Editor::new(&self.preview).readonly(true).h_full()))
                            .child(h_flex().gap_1().children(expansion.stops.into_iter().map(|stop|{
                                let range=stop.range();Button::new(("preview-stop",stop.index as usize)).ghost().xsmall().label(if stop.index==0{"Cursor".into()}else{format!("{}",stop.index)}).tooltip("Highlight in preview").on_click(cx.listener(move|this,_,_,cx|this.preview.update(cx,|e,cx|e.set_selected_range(range.clone(),cx))))
                            }))))))
                    .when_some(validation.or(self.error.clone()),|view,e|view.child(div().text_sm().text_color(theme.danger).child(e)))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(self.status.clone())))
                .when(self.ai.open,|row|row.child(self.render_ai(window,cx))))
    }
}

#[cfg(feature="gui-test")]
#[path="fixture.rs"]
pub(crate) mod fixture;
