//! Dedicated snippet agent with staged files, native questions, review, and Undo.
use gpui_kit::*;
use gpui_kit::component::input::{Input,InputState,InputEvent};
use gpui_kit::component::button::{Button,ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _,Disableable as _,Sizable as _,h_flex,v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::assets::IconName;
use practice::{agents::{self,Cancel,Event,Request,Access,Selection},snippets::{Snippet,store}};
use futures::StreamExt as _;
use super::editor::SnippetEditor;
use crate::assist::Target;

pub(super) struct State {
    pub open:bool,
    input:Entity<InputState>,
    busy:bool,
    cancel:Cancel,
    reply:String,
    history:String,
    session:Option<String>,
    target:Option<(agents::Detected,Option<Selection>)>,
    pub(super) stage:Option<std::path::PathBuf>,
    pub(super) base:Vec<Snippet>,
    pub(super) proposed:Option<Vec<Snippet>>,
    question:Option<Entity<crate::agent_question::QuestionForm>>,
    pub undo:Option<(Vec<Snippet>,Vec<Snippet>)>,
    _subscription:Subscription,
}
impl State {
    pub fn new(window:&mut Window,cx:&mut Context<SnippetEditor>)->Self{
        let input=cx.new(|cx|InputState::new(window,cx).placeholder("What would you like to create?"));
        let subscription=cx.subscribe(&input,|_,_,_:&InputEvent,cx|cx.notify());
        Self{open:false,input,busy:false,cancel:Cancel::default(),reply:String::new(),history:String::new(),session:None,target:None,stage:None,base:vec![],proposed:None,question:None,undo:None,_subscription:subscription}
    }
}
impl Drop for State{fn drop(&mut self){self.cancel.cancel();}}
enum Msg{Event(Event),Done(anyhow::Result<agents::Outcome>)}
impl SnippetEditor{
    fn send_ai(&mut self,window:&mut Window,cx:&mut Context<Self>){
        if self.ai.busy{return;}
        let text=self.ai.input.read(cx).value().trim().to_owned();if text.is_empty(){return;}
        let Some(ws)=self.workspace.upgrade()else{return;};
        let Target::Agent(agent,selection)=ws.read(cx).assist.read(cx).target(&ws.read(cx).config)else{self.error=Some("Choose a local agent in AI settings".into());cx.notify();return;};
        let target=(agent.clone(),selection.clone());
        if self.ai.target.as_ref()!=Some(&target){self.ai.session=None;self.ai.history.clear();self.ai.target=Some(target);}
        if self.ai.stage.is_none(){
            let path=self.dir.join(format!(".agent-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
            if let Err(e)=store::save(&path,&self.user){self.error=Some(e.to_string());cx.notify();return;}
            self.ai.base=self.user.clone();self.ai.stage=Some(path);
        }
        let stage=self.ai.stage.clone().unwrap();let draft=self.draft(cx);let language=self.language;
        self.ai.history.push_str(&format!("\nUser: {text}\n"));
        let history=self.ai.history.clone();
        let prompt=format!("You are Leet's snippet editor agent. {}\nThe ONLY editable snippet library for this conversation is {:?}. Always pass --directory {:?} to the snippet tool. Work on this staged copy; the user reviews and applies changes in the app. Do not touch the real library or any other files. Start by asking the user's preferences with ACP elicitation/create if available; otherwise ask a concise question in your reply and wait for the next turn. Ask only for missing language, style, trigger and cursor information. You can list/show/set/remove through the tool, create several snippets and revise existing snippets. Set accepts all fields: name, prefixes (string array), body, description, scope (python/cpp/go/c/java), template (bool), origin (ai). Always choose a specific language; create separate snippets when supporting multiple languages. Return plain conversational text. Current language: {}. Selected snippet: {}. Conversation: {}",practice::snippets::cli::context(),stage,stage,language.id(),serde_json::to_string(&draft).unwrap_or_default(),history);
        let resume=self.ai.session.clone();let cancel=Cancel::default();self.ai.cancel=cancel.clone();self.ai.busy=true;self.ai.reply.clear();self.ai.proposed=None;self.ai.question=None;
        self.ai.input.update(cx,|e,cx|e.set_value("",window,cx));
        let(tx,mut rx)=futures::channel::mpsc::unbounded();
        let job=cx.background_spawn(async move{
            let result=(||->anyhow::Result<_>{
                let selection=match selection{Some(s)=>s,None=>{let catalog=agents::catalog(&agent)?;Selection{agent:agent.kind,model:catalog.default_model.ok_or_else(||anyhow::anyhow!("Agent has no default model"))?,effort:None,fast:false}}};
                let request=Request{agent,selection,prompt,schema:None,cwd:stage,access:Access::Full,resume};
                agents::run(&request,&mut|event|{let _=tx.unbounded_send(Msg::Event(event));},&cancel)
            })();let _=tx.unbounded_send(Msg::Done(result));
        });
        cx.spawn_in(window,async move|this,cx|{
            while let Some(msg)=rx.next().await{let _=this.update_in(cx,|this,window,cx|{
                match msg{
                    Msg::Event(Event::Text(text))=>this.ai.reply.push_str(&text),
                    Msg::Event(Event::Question(q))=>this.ai.question=Some(cx.new(|cx|crate::agent_question::QuestionForm::new(q,window,cx))),
                    Msg::Event(_)=>{},
                    Msg::Done(result)=>{
                        this.ai.busy=false;
                        match result{
                            Ok(outcome)=>{
                                this.ai.session=outcome.session;this.ai.reply=outcome.text;
                                this.ai.history.push_str(&format!("Agent: {}\n",this.ai.reply));
                                let loaded=store::load(this.ai.stage.as_ref().unwrap());
                                if !loaded.errors.is_empty(){this.error=Some(format!("Invalid agent snippets: {:?}",loaded.errors));}
                                else if loaded.snippets!=this.ai.base{this.ai.proposed=Some(loaded.snippets);}
                            }
                            Err(e)=>this.error=Some(e.to_string()),
                        }
                    }
                }cx.notify();
            });}job.await;
        }).detach();cx.notify();
    }
    fn apply_ai(&mut self,window:&mut Window,cx:&mut Context<Self>){
        if !self.save(window,cx){return;}
        let Some(proposed)=self.ai.proposed.clone()else{return;};
        if let Err(e)=store::commit(&self.dir,&self.ai.base,&proposed){self.error=Some(e.to_string());cx.notify();return;}
        self.ai.undo=Some((self.ai.base.clone(),proposed.clone()));self.ai.base=proposed;self.ai.proposed=None;
        self.original=None;self.refresh(window,cx);self.notify_workspace(window,cx);cx.notify();
    }
    fn undo_ai(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let Some((before,after))=self.ai.undo.clone()else{return;};
        if let Err(e)=store::commit(&self.dir,&after,&before){self.error=Some(e.to_string());cx.notify();return;}
        self.ai.undo=None;self.ai.stage=None;self.ai.session=None;self.ai.history.clear();self.original=None;
        self.refresh(window,cx);self.notify_workspace(window,cx);cx.notify();
    }
    pub(super) fn render_ai(&self,window:&mut Window,cx:&mut Context<Self>)->AnyElement{
        let theme=cx.theme().clone();
        let changed:Vec<_>=self.ai.proposed.as_ref().map(|p|p.iter().filter(|s|!self.ai.base.contains(s)).cloned().collect()).unwrap_or_default();
        let removed:Vec<_>=self.ai.proposed.as_ref().map(|p|self.ai.base.iter().filter(|s|!p.iter().any(|new|new.key()==s.key())).map(|s|s.name.clone()).collect()).unwrap_or_default();
        v_flex().w(px(if window.viewport_size().width < px(1000.) {240.}else{300.})).min_h_0().p_3().gap_2().rounded_md().bg(theme.sidebar)
            .child(h_flex().justify_between().child(div().text_sm().child("Snippet assistant"))
                .child(Button::new("snippet-agent-settings").ghost().small().icon(IconName::Settings).tooltip("Choose agent and model").on_click(cx.listener(|this,_,w,cx|{let ws=this.workspace.clone();w.defer(cx,move|w,cx|{let _=ws.update(cx,|ws,cx|ws.ai_chip.update(cx,|chip,cx|chip.open(w,cx)));});}))))
            .child(v_flex().id("snippet-ai-reply").overflow_y_scroll().flex_1().min_h_0().gap_2()
                .child(div().text_sm().child(if self.ai.reply.is_empty(){"Ask for templates, algorithms or edits.".into()}else{self.ai.reply.clone()}))
                .children(self.ai.question.clone())
                .children(removed.into_iter().map(|name|div().text_sm().text_color(theme.muted_foreground).child(format!("Remove {name}"))))
                .children(changed.into_iter().map(|s|Button::new(SharedString::from(format!("review-ai-{}",s.key()))).outline().small().label(format!("Review {}",s.name)).on_click(cx.listener(move|this,_,w,cx|this.stage(s.clone(),w,cx)))))
                .when(self.ai.proposed.is_some(),|v|v.child(Button::new("apply-ai-snippets").primary().small().label("Apply changes").on_click(cx.listener(|this,_,w,cx|this.apply_ai(w,cx)))))
                .when(self.ai.undo.is_some(),|v|v.child(Button::new("undo-ai-snippets").ghost().small().label("Undo changes").on_click(cx.listener(|this,_,w,cx|this.undo_ai(w,cx))))))
            .child(Input::new(&self.ai.input).small())
            .child(Button::new("send-snippet-ai").primary().small().icon(if self.ai.busy{IconName::Square}else{IconName::ArrowUp}).label(if self.ai.busy{"Stop"}else{"Send"}).disabled(!self.ai.busy&&self.ai.input.read(cx).value().trim().is_empty())
                .on_click(cx.listener(|this,_,w,cx|{if this.ai.busy{this.ai.cancel.cancel();}else{this.send_ai(w,cx);}})))
            .into_any_element()
    }
}
