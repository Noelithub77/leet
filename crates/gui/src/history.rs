//! Account-scoped remote versions alongside local commits and the staged draft.
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::assets::IconName;
use gpui_kit::component::notification::Notification;
use gpui_kit::prelude::FluentBuilder as _;
use serde::{Deserialize, Serialize};
use practice::{creds::{self, Account}, git, language::Language};
use crate::workspace::Workspace;

#[derive(Clone, Serialize, Deserialize)]
pub struct RemoteVersion { pub id:String, pub label:String, pub language:String, pub time:i64, pub code:Option<String>, pub provider:String }
#[derive(Clone)]
pub struct LocalVersion { pub commit:git::Commit, pub rel:std::path::PathBuf, pub language:Language }
pub fn local_versions(workspace:&std::path::Path,frontend_id:u32,slug:&str)->Vec<LocalVersion>{
    Language::ALL.into_iter().flat_map(|language|{
        let rel=practice::workspace::solution_rel(frontend_id,slug,language);
        git::history(workspace,&rel).unwrap_or_default().into_iter().map(move|commit|LocalVersion{commit,rel:rel.clone(),language})
    }).collect()
}
#[derive(Clone)]
pub enum Version { Draft(String), Commit(LocalVersion), Remote(RemoteVersion) }
impl Version {
    fn time(&self)->i64 { match self { Self::Draft(_)=>i64::MAX,Self::Commit(version)=>version.commit.time,Self::Remote(version)=>version.time } }
    fn label(&self)->String { match self { Self::Draft(_)=>"Staged draft".into(),Self::Commit(version)=>format!("{} · {}",version.commit.summary,version.language.label()),Self::Remote(version)=>format!("{} · {}",version.label,version.language) } }
    fn provider(&self)->&str { match self { Self::Draft(_)|Self::Commit(_)=>"Git", Self::Remote(version)=>&version.provider } }
}
impl Workspace {
    pub fn clear_remote_history(&mut self) {
        for session in self.session.iter_mut().chain(self.tabs.iter_mut().flatten().map(|tab| &mut tab.session)) {
            session.remote_versions.clear(); session.history_loaded=false; session.history_loading=false; session.history_status=None; session.version_sequence+=1; session.history_sequence+=1;
        }
    }
    pub fn versions(&self)->Vec<Version> {
        let Some(session)=&self.session else { return vec![]; };
        let mut versions:Vec<_>=session.history.iter().cloned().map(Version::Commit).chain(session.remote_versions.iter().cloned().map(Version::Remote)).collect();
        if let Ok(Some(code))=git::staged_code(&self.config.workspace,&session.rel){versions.push(Version::Draft(code));}
        versions.sort_by_key(|version|std::cmp::Reverse(version.time())); versions
    }
    pub fn load_remote_versions(&mut self, window:&mut Window,cx:&mut Context<Self>) {
        let Some(session)=self.session.as_mut() else{return;};
        if session.history_loading{return;}
        session.history_loading=true; session.history_status=None; session.history_sequence+=1; let sequence=session.history_sequence;
        let slug=session.slug.clone(); let db=self.db.clone(); let client=self.client.clone();
        cx.spawn_in(window,async move|this,cx|{
            let fetch_slug=slug.clone();
            let result=cx.background_spawn(async move{
                let mut versions=vec![]; let mut status=vec![];
                if !fetch_slug.starts_with("cf:") {
                    if let Some(identity)=client.cache_identity(){
                        let key=format!("submissions:{identity}:{fetch_slug}");
                        let cached=db.get(&key)?.and_then(|raw|serde_json::from_str::<Vec<RemoteVersion>>(&raw).ok());
                        match client.submissions(&fetch_slug){
                            Ok(items)=>{
                                versions.extend(items.into_iter().map(|item|RemoteVersion{id:item.id,label:item.status,language:item.lang,time:item.timestamp.parse().unwrap_or(0),code:None,provider:"LeetCode".into()}));
                                db.set(&key,&serde_json::to_string(&versions)?)?;
                            },
                            Err(error)=>{if let Some(cached)=cached{versions.extend(cached);}status.push(error.to_string());}
                        }
                    }else{status.push("Sign in to LeetCode to load submissions".into());}
                    if let Some(entry)=practice::roadmap::entry(&fetch_slug){
                        if let Ok(account)=creds::load(Account::NeetCode){
                            let key=format!("saved-code:{}:{fetch_slug}",account.user_id);
                            let cached=db.get(&key)?.and_then(|raw|serde_json::from_str::<Vec<RemoteVersion>>(&raw).ok());
                            match practice::neetcode::Client::default().saved_code(&entry.nc){
                                Ok((data,_))=>{let saved=saved_versions(&data);db.set(&key,&serde_json::to_string(&saved)?)?;versions.extend(saved);},
                                Err(error)=>{if let Some(cached)=cached{versions.extend(cached);}status.push(error.to_string());}
                            }
                        }
                    }
                }
                anyhow::Ok((versions,status.join(" · ")))
            }).await;
            let _=this.update(cx,|ws,cx|{
                let Some(session)=ws.session_for_mut(&slug)else{return;};if session.history_sequence!=sequence{return;}session.history_loading=false;session.history_loaded=true;
                match result{Ok((versions,status))=>{session.remote_versions=versions;session.history_status=(!status.is_empty()).then_some(status);},Err(error)=>session.history_status=Some(error.to_string())};cx.notify();
            });
        }).detach();cx.notify();
    }
    pub fn restore_version(&mut self,index:usize,window:&mut Window,cx:&mut Context<Self>){
        let Some(version)=self.versions().get(index).cloned()else{return;};
        let Some(session)=self.session.as_mut() else{return;};session.version_sequence+=1;let sequence=session.version_sequence;let slug=session.slug.clone(); let language=session.language;
        let workspace=self.config.workspace.clone(); let client=self.client.clone();let db=self.db.clone();
        let identity=client.cache_identity();
        cx.spawn_in(window,async move|this,cx|{
            let result: anyhow::Result<(String, Language)>=cx.background_spawn(async move{
                match version{
                    Version::Draft(code)=>Ok((code,language)),
                    Version::Commit(version)=>Ok((git::file_at(&workspace,&version.commit.id,&version.rel)?,version.language)),
                    Version::Remote(version)=>{
                        let language=version_language(&version.language).ok_or_else(||anyhow::anyhow!("This version's language is not supported in the editor"))?;
                        let key=format!("submission-code:{}:{}",identity.unwrap_or_default(),version.id);
                        let code=if let Some(code)=version.code{code}else if let Some(code)=db.get(&key)?{code}else{let code=client.submission_code(&version.id)?;db.set(&key,&code)?;code};Ok((code,language))
                    }
                }
            }).await;
            let _=this.update_in(cx,|ws,window,cx|{
                if ws.session.as_ref().is_none_or(|session|session.slug!=slug||session.version_sequence!=sequence){return;}
                match result{Ok((code,language))=>ws.restore_version_code(code,language,window,cx),Err(error)=>ws.toast(Notification::error(error.to_string()),window,cx)}
            });
        }).detach();
    }
    fn restore_version_code(&mut self,code:String,language:Language,window:&mut Window,cx:&mut Context<Self>){
        let Some(session)=&self.session else{return;};let current=self.editor.read(cx).value().to_string();
        if let Err(error)=git::stage(&self.config.workspace,&session.rel,&current){self.toast(Notification::error(format!("Could not stage current draft: {error}")),window,cx);return;}
        if session.language!=language{
            let rel=practice::workspace::solution_rel(session.frontend_id,&session.slug,language);let path=self.config.workspace.join(&rel);
            if let Some(parent)=path.parent(){if let Err(error)=std::fs::create_dir_all(parent){self.toast(Notification::error(error.to_string()),window,cx);return;}}
            let statement=self.statement.clone();let preferred=self.config.preferred_language;self.config.preferred_language=language;self.new_editor(window,cx);self.config.preferred_language=preferred;self.statement=statement;
            if let Some(session)=self.session.as_mut(){session.language=language;session.rel=rel;session.path=path;}
        }
        self.statement.update(cx,|statement,cx|{ statement.language=language; statement.article=None; statement.reference=None; cx.notify(); });
        self.editor.update(cx,|editor,cx|editor.replace_all(code,window,cx));self.save_now(cx);self.attach_language_server(window,cx);
        self.focus_editor(window,cx);self.toast(Notification::success("Version restored · current draft staged"),window,cx);cx.notify();
    }
    pub fn render_versions(&self,cx:&mut Context<Self>)->impl IntoElement{
        let theme=cx.theme().clone();let versions=self.versions();let session=self.session.as_ref();
        v_flex().id("question-versions").size_full().overflow_y_scroll().p_3().gap_2()
            .child(h_flex().justify_between().child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("History"))
                .child(Button::new("refresh-versions").ghost().xsmall().icon(IconName::RefreshCw).tooltip("Refresh submissions").accessibility_label("Refresh submissions").on_click(cx.listener(|this,_,window,cx|this.load_remote_versions(window,cx)))))
            .when(session.is_some_and(|session|session.history_loading),|view|view.child(div().text_xs().text_color(theme.muted_foreground).child("Loading submissions…")))
            .when_some(session.and_then(|session|session.history_status.clone()),|view,status|view.child(div().text_xs().text_color(theme.muted_foreground).child(status)))
            .when(versions.is_empty(),|view|view.child(div().text_color(theme.muted_foreground).text_sm().child("No versions yet")))
            .children(versions.into_iter().enumerate().map(|(index,version)|{
                let selected=session.is_some_and(|session|session.selected_commit==index);
                v_flex().id(("version",index)).rounded_lg().p_3().gap_1().border_1().border_color(if selected{rgb(0x8be9fd).into()}else{theme.border})
                    .when(selected,|view|view.bg(rgb(0x8be9fd).opacity(0.08)))
                    .child(h_flex().justify_between().child(div().truncate().child(version.label())).child(div().text_xs().text_color(theme.muted_foreground).child(version.provider().to_owned())))
                    .when(version.time()!=i64::MAX,|view|view.child(div().text_xs().text_color(theme.muted_foreground).child(crate::view::relative_time(version.time()))))
                    .on_click(cx.listener(move|this,_,window,cx|{if let Some(session)=this.session.as_mut(){session.selected_commit=index;}this.restore_version(index,window,cx);}))
            }))
    }
}
fn version_language(language:&str)->Option<Language>{match language.to_lowercase().as_str(){"python"|"python3"=>Some(Language::Python),"cpp"|"c++"=>Some(Language::Cpp),"go"|"golang"=>Some(Language::Go),"c"=>Some(Language::C),"java"=>Some(Language::Java),_=>None}}
fn saved_versions(data:&serde_json::Value)->Vec<RemoteVersion>{
    fn collect(data:&serde_json::Value,language:&str,out:&mut Vec<RemoteVersion>){
        if let Some(object)=data.as_object(){
            if let Some(code)=object.get("code").and_then(serde_json::Value::as_str){
                let language=object.get("lang").or_else(||object.get("language")).and_then(serde_json::Value::as_str).unwrap_or(language);
                if version_language(language).is_some(){out.push(RemoteVersion{id:format!("nc:{language}:{}",out.len()),label:object.get("name").or_else(||object.get("title")).and_then(serde_json::Value::as_str).unwrap_or("Saved code").into(),language:language.into(),time:object.get("updatedAt").and_then(serde_json::Value::as_i64).unwrap_or(0),code:Some(code.into()),provider:"NeetCode".into()});}return;
            }
            for(key,value)in object{collect(value,if version_language(key).is_some(){key}else{language},out);}
        }else if let Some(values)=data.as_array(){for value in values{collect(value,language,out);}}
    }
    let mut out=vec![];collect(data,"",&mut out);out
}
#[cfg(test)]mod tests{use super::*;#[::core::prelude::v1::test]fn saved_cloud_tabs_keep_language_and_code(){let versions=saved_versions(&serde_json::json!({"python":{"tabs":[{"name":"Attempt 1","code":"return 1"},{"name":"Attempt 2","code":"return 2"}]},"cpp":{"tabs":[{"code":"return 3;"}]}}));assert_eq!(versions.len(),3);assert!(versions.iter().any(|version|version.label=="Attempt 2"&&version.code.as_deref()==Some("return 2")));}}
