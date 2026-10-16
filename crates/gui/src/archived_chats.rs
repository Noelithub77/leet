//! Archived conversations managed from the universal settings page.
use std::sync::Arc;
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use practice::{chat::history::Thread, db::Db};

pub(crate) fn open(db: Arc<Db>, assist: WeakEntity<crate::assist::Assist>, window: &mut Window, cx: &mut App) {
    let view = cx.new(|_| ArchivedChats { db, assist, chats: vec![], pending_delete: None, error: None });
    view.update(cx, |view, _| view.reload());
    window.open_dialog(cx, move |dialog, _, _| dialog.title("Archived chats").w(px(600.)).child(view.clone()));
}

struct ArchivedChats {
    db: Arc<Db>,
    assist: WeakEntity<crate::assist::Assist>,
    chats: Vec<(Thread, String)>,
    pending_delete: Option<Thread>,
    error: Option<String>,
}

impl ArchivedChats {
    fn reload(&mut self) {
        match self.db.archived_chat_threads() {
            Ok(chats) => self.chats = chats.into_iter().map(|thread| {
                let problem = thread.problem.as_deref().unwrap_or_default();
                let title = self.db.question(problem).ok().flatten().map(|question| question.title)
                    .or_else(|| self.db.chat_messages(thread.id).ok()?.into_iter().map(|message| message.turn.problem_title).find(|title| !title.is_empty()))
                    .unwrap_or_else(|| problem.to_owned());
                (thread, title)
            }).collect(),
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn restore(&mut self, id: i64, cx: &mut Context<Self>) {
        match self.db.set_chat_archived(id, false) {
            Ok(()) => { self.error = None; self.reload(); let _ = self.assist.update(cx, |assist, cx| { assist.refresh_threads(); cx.notify(); }); }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    fn delete(&mut self, id: i64, cx: &mut Context<Self>) {
        let result = self.db.chat_messages(id).and_then(|messages| {
            self.db.delete_chat_permanently(id)?;
            let _ = self.assist.update(cx, |assist, cx| { assist.forget_thread(id); assist.refresh_threads(); cx.notify(); });
            for message in messages {
                let directory = practice::chat::artifacts::directory(message.id);
                if directory.exists() { std::fs::remove_dir_all(directory)?; }
            }
            Ok(())
        });
        self.error = result.err().map(|error| error.to_string());
        self.pending_delete = None; self.reload(); cx.notify();
    }
}

impl Render for ArchivedChats {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex().id("archived-chats").test_support().gap_3()
            .when_some(self.error.clone(), |view, error| view.child(div().text_sm().text_color(theme.danger).child(error)))
            .map(|view| if let Some(thread) = self.pending_delete.clone() {
                let id = thread.id;
                view.child(div().text_sm().child(format!("Delete “{}” permanently? This cannot be undone.", thread.title)))
                    .child(h_flex().justify_end().gap_2()
                        .child(Button::new("cancel-delete-chat").label("Cancel").on_click(cx.listener(|this, _, _, cx| { this.pending_delete = None; cx.notify(); })))
                        .child(Button::new("confirm-delete-chat").danger().label("Delete permanently").on_click(cx.listener(move |this, _, _, cx| this.delete(id, cx)))))
            } else {
                view.when(self.chats.is_empty(), |view| view.child(div().text_sm().text_color(theme.muted_foreground).child("No archived chats")))
                    .when(!self.chats.is_empty(), |view| view.child(uniform_list("archived-chat-list", self.chats.len(), cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range.map(|index| {
                            let (thread, problem) = &this.chats[index]; let id = thread.id; let deleting = thread.clone();
                            h_flex().h(px(64.)).w_full().gap_3().border_b_1().border_color(cx.theme().border)
                                .child(v_flex().flex_1().min_w_0().gap_1()
                                    .child(div().text_sm().truncate().child(thread.title.clone()))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).truncate().child(problem.clone())))
                                .child(Button::new(("restore-chat", id as u64)).ghost().small().label("Restore").on_click(cx.listener(move |this, _, _, cx| this.restore(id, cx))))
                                .child(Button::new(("delete-chat", id as u64)).ghost().small().icon(gpui_kit::assets::IconName::Trash).tooltip("Delete permanently").accessibility_label("Delete permanently")
                                    .on_click(cx.listener(move |this, _, _, cx| { this.pending_delete = Some(deleting.clone()); cx.notify(); }))).into_any_element()
                        }).collect()
                    })).w_full().h(px(self.chats.len().min(5) as f32 * 64.))))
            })
    }
}
