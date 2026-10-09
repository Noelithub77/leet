//! Compact native forms for ACP elicitation in chat and the snippet editor.
use gpui_kit::*;
use gpui_kit::component::input::{Input,InputState};
use gpui_kit::component::button::{Button,ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _,Sizable as _,Disableable as _,h_flex,v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use serde_json::{Value,json};
use practice::agents::Question;
struct Field {key:String,label:String,schema:Value,input:Entity<InputState>}
pub struct QuestionForm {question:Question,fields:Vec<Field>,error:Option<String>,unsupported:bool,answered:bool}
impl QuestionForm {
    pub fn new(question:Question,window:&mut Window,cx:&mut Context<Self>)->Self {
        let fields=question.request["requestedSchema"]["properties"].as_object().map(|p|p.iter().take(12).map(|(key,schema)|Field{
            key:key.clone(),label:schema["title"].as_str().unwrap_or(key).into(),schema:schema.clone(),input:cx.new(|cx|InputState::new(window,cx).default_value(schema.get("default").map(|v|v.as_str().map(str::to_owned).unwrap_or_else(||v.to_string())).unwrap_or_default()))
        }).collect()).unwrap_or_default();
        let unsupported=question.request["requestedSchema"]["properties"].as_object().is_none_or(|p|p.len()>12 || p.values().any(|s|!matches!(s["type"].as_str(),Some("string"|"boolean"|"integer"|"number"|"array"))));
        Self{question,fields,error:unsupported.then(||"This question needs an unsupported form. Skip to answer in chat.".into()),unsupported,answered:false}
    }
    fn submit(&mut self,cx:&mut Context<Self>){
        if self.unsupported { return; }
        let mut content=serde_json::Map::new();
        for f in &self.fields {
            let text=f.input.read(cx).value();let required=self.question.request["requestedSchema"]["required"].as_array().is_some_and(|a|a.iter().any(|v|v==&f.key));
            if text.is_empty() && !required {continue;}
            let parsed=match f.schema["type"].as_str(){
                Some("boolean"|"integer"|"number"|"array")=>serde_json::from_str::<Value>(&text).ok(),
                _=>Some(json!(text.as_str())),
            };
            let valid=parsed.filter(|value|{
                let correct=match f.schema["type"].as_str(){Some("boolean")=>value.is_boolean(),Some("integer")=>value.as_i64().is_some(),Some("number")=>value.is_number(),Some("array")=>value.is_array(),_=>value.is_string()};
                let options=f.schema.get("enum").and_then(Value::as_array);let enums=options.is_none_or(|options|options.contains(value));
                let bounds=value.as_f64().is_none_or(|n|f.schema["minimum"].as_f64().is_none_or(|min|n>=min)&&f.schema["maximum"].as_f64().is_none_or(|max|n<=max));
                let length=value.as_str().is_none_or(|s|f.schema["minLength"].as_u64().is_none_or(|min|s.chars().count()>=min as usize)&&f.schema["maxLength"].as_u64().is_none_or(|max|s.chars().count()<=max as usize));
                correct&&enums&&bounds&&length&&(!required||!text.is_empty())
            });
            let Some(value)=valid else{self.error=Some(format!("Check {}",f.label));cx.notify();return;};content.insert(f.key.clone(),value);
        }
        self.question.answer(json!({"action":"accept","content":content}));self.answered=true;cx.notify();
    }
}
impl Drop for QuestionForm{fn drop(&mut self){if !self.answered{self.question.answer(json!({"action":"cancel"}));}}}
impl Render for QuestionForm{
    fn render(&mut self,_:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
        let theme=cx.theme().clone();
        v_flex().p_3().gap_2().rounded_md().bg(theme.muted)
            .child(div().text_sm().child(if self.answered{"Answer sent".into()}else{self.question.request["message"].as_str().unwrap_or("Your preferences").to_owned()}))
            .when(!self.answered,|v|v.children(self.fields.iter().map(|f|{
                let input=f.input.clone();let options=f.schema["enum"].as_array().cloned().unwrap_or_else(||if f.schema["type"]=="boolean"{vec![json!(true),json!(false)]}else{Vec::new()});
                v_flex().gap_1().child(div().text_xs().child(f.label.clone()))
                    .when(!options.is_empty(),|v|v.child(h_flex().flex_wrap().gap_1().children(options.into_iter().map(|value|{let input=input.clone();let text=value.as_str().map(str::to_owned).unwrap_or_else(||value.to_string());Button::new(SharedString::from(format!("question-{}-{text}",f.key))).outline().small().label(text.clone()).on_click(move|_,w,cx|input.update(cx,|i,cx|i.set_value(text.clone(),w,cx))) }))))
                    .child(Input::new(&f.input).small())
            })).when_some(self.error.clone(),|v,e|v.child(div().text_sm().text_color(theme.danger).child(e)))
                .child(h_flex().gap_2().child(Button::new("answer-agent-question").primary().small().label("Continue").disabled(self.unsupported).on_click(cx.listener(|this,_,_,cx|this.submit(cx))))
                    .child(Button::new("decline-agent-question").ghost().small().label("Skip").on_click(cx.listener(|this,_,_,cx|{this.question.answer(json!({"action":"decline"}));this.answered=true;cx.notify();})))))
    }
}

#[cfg(feature="gui-test")]
pub(crate) fn fixture(cx:&mut HeadlessAppContext)->anyhow::Result<serde_json::Value>{
    use gpui_kit::test::TestWindowExt as _;
    let (handle,form)=cx.update(|cx|gpui_kit::open_window(WindowOptions{show:false,focus:false,..Default::default()},cx,|window,cx|cx.new(|cx|QuestionForm::new(Question::new(json!({"message":"Snippet preferences","requestedSchema":{"type":"object","properties":{"language":{"type":"string","enum":["cpp","python"]},"tests":{"type":"boolean"}},"required":["language","tests"]}})),window,cx))))?;
    let handle=handle.into();cx.run_until_parked();
    cx.update_window(handle,|_,window,cx|->anyhow::Result<()>{
        window.render_frame(cx);window.click("answer-agent-question",cx);
        anyhow::ensure!(!form.read(cx).answered,"Empty required preferences were accepted");
        window.render_frame(cx);window.click("question-language-cpp",cx);window.click("question-tests-true",cx);
        window.render_frame(cx);window.click("answer-agent-question",cx);
        anyhow::ensure!(form.read(cx).answered,"Native preferences were not submitted");
        window.remove_window();Ok(())
    })??;
    Ok(json!({"fixture":"agent-question","passed":true,"checks":["required preference validation","enum and boolean choices","native submission"]}))
}
