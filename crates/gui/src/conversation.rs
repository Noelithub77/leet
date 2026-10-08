//! Thread selection, history mutations, drafts, and editable response artifacts.
use super::*;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;

impl Assist {
    pub(super) fn selection_key(&self) -> String { format!("chat:selected:{}", if self.root_scope { "root" } else { self.slug.as_deref().unwrap_or("root") }) }
    pub(super) fn refresh_threads(&mut self) {
        match self.db.visible_chat_threads(self.slug.as_deref(), &self.thread_query) {
            Ok(threads) => { self.threads = threads; self.thread_selection = self.thread_selection.min(self.threads.len().saturating_sub(1)); },
            Err(error) => self.conversation_error = Some(error.to_string()),
        }
    }
    pub(super) fn load_thread(&mut self, id: i64) {
        if !self.loaded_threads.contains(&id) {
            match self.db.chat_messages(id) {
                Ok(messages) => {
                    for message in messages {
                        self.restore_message(message);
                    }
                    self.loaded_threads.insert(id);
                }
                Err(error) => { self.conversation_error = Some(error.to_string()); return; }
            }
        }
        self.refresh_artifacts(id);
        self.root_scope = self.db.chat_thread(id).is_ok_and(|thread| thread.problem.is_none());
        if let Err(error) = self.db.set("chat:scope", if self.root_scope { "root" } else { "problem" }) { self.conversation_error = Some(error.to_string()); }
        self.selected_thread = Some(id);
        if let Err(error) = self.db.set(&self.selection_key(), &id.to_string()) { self.conversation_error = Some(error.to_string()); }
    }
    pub(super) fn restore_message(&mut self, message: practice::chat::history::Message) {
        if self.runs.iter().any(|run| run.message_id == message.id) { return; }
        let turn = message.turn; let action = turn.action(); let run_id = self.next_id();
        let mut run = Run::new(run_id, turn.problem, action, turn.agent, turn.model, None, None);
        run.thread_id = message.thread_id; run.message_id = message.id; run.problem_title = turn.problem_title;
        run.request_prompt = if turn.request_prompt.is_empty() { action.instructions().to_owned() } else { turn.request_prompt };
        run.instructions = turn.instructions; run.elapsed = Some(Duration::ZERO);
        run.phase = match turn.phase { practice::chat::history::SavedPhase::Done => Phase::Done, practice::chat::history::SavedPhase::Stopped => Phase::Stopped, practice::chat::history::SavedPhase::Failed(error) => Phase::Failed(error) };
        run.set_answer(turn.answer); self.runs.push(run);
        self.runs.sort_by_key(|run| std::cmp::Reverse(run.message_id));
    }
    pub(super) fn ensure_thread(&mut self) -> anyhow::Result<i64> {
        if let Some(id) = self.selected_thread { return Ok(id); }
        let problem = if self.root_scope { None } else { self.slug.as_deref() };
        let thread = self.db.create_chat(problem, "New chat")?;
        self.selected_thread = Some(thread.id); self.loaded_threads.insert(thread.id); self.refresh_threads();
        self.db.set("chat:scope", if self.root_scope { "root" } else { "problem" })?;
        self.db.set(&self.selection_key(), &thread.id.to_string())?;
        Ok(thread.id)
    }
    pub(super) fn open_thread_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_threads = true;
        self.thread_selection = self.threads.iter().position(|thread| Some(thread.id) == self.selected_thread).unwrap_or(0);
        self.thread_search.focus_handle(cx).focus(window, cx);
        cx.notify();
    }
    pub(super) fn select_thread(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.persist_draft(cx); if self.editing.take().is_some() { self.composer_thread = None; }
        self.load_thread(id); self.sync_composer(window, cx); self.show_threads = false; self.composer.focus_handle(cx).focus(window, cx); cx.notify();
    }
    pub(super) fn persist_draft(&mut self, cx: &App) {
        if self.editing.is_some() { return; }
        if let Some(id) = self.composer_thread {
            if let Err(error) = self.db.save_chat_draft(id, self.composer.read(cx).value().as_ref()) { self.conversation_error = Some(error.to_string()); }
        }
    }
    pub(super) fn new_thread(&mut self, root: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.persist_draft(cx); self.root_scope = root; self.selected_thread = None; self.editing = None;
        match self.ensure_thread() {
            Ok(_) => { self.sync_composer(window, cx); self.ai_tab = 2; self.show_threads = false; }
            Err(error) => self.conversation_error = Some(error.to_string()),
        }
        cx.notify();
    }
    pub(super) fn fork_message(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter().find(|run| run.id == id) else { return };
        match self.db.fork_chat(run.thread_id, run.message_id, true) {
            Ok(thread) => { self.refresh_threads(); self.select_thread(thread.id, window, cx); }
            Err(error) => { self.conversation_error = Some(error.to_string()); cx.notify(); }
        }
    }
    pub(super) fn edit_message(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter().find(|run| run.id == id) else { return };
        if run.phase.active() || run.phase == Phase::Confirm { return; }
        let text = if run.action == Action::Ask { run.instructions.clone() } else { format!("{}{}", run.request_prompt, if run.instructions.is_empty() { String::new() } else { format!("\n\n{}", run.instructions) }) };
        self.persist_draft(cx); self.editing = Some(id); self.composer.update(cx, |input, cx| input.set_value(text, window, cx));
        self.composer.focus_handle(cx).focus(window, cx); cx.notify();
    }
    pub(super) fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = None; self.composer_thread = None; self.sync_composer(window, cx); cx.notify();
    }
    fn navigate_threads(&mut self, direction: isize, cx: &mut Context<Self>) {
        self.thread_selection = self.thread_selection.saturating_add_signed(direction).min(self.threads.len().saturating_sub(1));
        self.thread_scroll.scroll_to_item(self.thread_selection, ScrollStrategy::Nearest); cx.notify();
    }
    pub(super) fn delete_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_thread else { return };
        self.persist_draft(cx);
        if let Err(error) = self.db.delete_chat(id) { self.conversation_error = Some(error.to_string()); cx.notify(); return; }
        for run in self.runs.iter().filter(|run| run.thread_id == id) { run.cancel.cancel(); }
        self.runs.retain(|run| run.thread_id != id); self.loaded_threads.remove(&id);
        self.selected_thread = None; self.composer_thread = None; self.refresh_threads();
        if let Some(thread) = self.threads.first() { self.load_thread(thread.id); }
        self.composer.update(cx, |input, cx| input.set_value("", window, cx)); self.sync_composer(window, cx);
        self.show_threads = true; self.editing = None; cx.notify();
    }
    pub(super) fn undo_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.persist_draft(cx);
        match self.db.undo_chat() {
            Ok(id) => {
                let existing: HashSet<_> = self.runs.iter().map(|run| run.thread_id).collect();
                for thread in existing {
                    if self.db.chat_thread(thread).is_err() {
                        for run in self.runs.iter().filter(|run| run.thread_id == thread) { run.cancel.cancel(); }
                        self.runs.retain(|run| run.thread_id != thread); self.loaded_threads.remove(&thread);
                    }
                }
                if let Some(id) = id && let Ok(thread) = self.db.chat_thread(id) {
                    self.root_scope = thread.problem.is_none();
                    self.refresh_threads(); self.select_thread(id, window, cx);
                } else { self.selected_thread = None; self.refresh_threads(); self.sync_composer(window, cx); }
                self.editing = None;
            }
            Err(error) => self.conversation_error = Some(error.to_string()),
        }
        cx.notify();
    }
    pub(super) fn refresh_artifacts(&mut self, thread: i64) {
        for run in self.runs.iter_mut().filter(|run| run.thread_id == thread && !run.phase.active()) {
            let path = practice::chat::artifacts::path(run.message_id);
            if !path.exists() && let Some(answer) = &run.answer {
                if let Err(error) = practice::chat::artifacts::save(&path, answer) { self.conversation_error = Some(error.to_string()); }
            }
            let modified = std::fs::metadata(&path).and_then(|metadata| metadata.modified()).ok();
            if modified.is_some() && modified != run.artifact_mtime {
                run.artifact_mtime = modified;
                match practice::chat::artifacts::load(&path) {
                    Ok(answer) => run.set_answer(Some(answer)),
                    Err(error) => self.conversation_error = Some(format!("Artifact: {error}")),
                }
            }
        }
    }
    pub(super) fn reload_artifact(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == id) else { return };
        match practice::chat::artifacts::load(&practice::chat::artifacts::path(run.message_id)) {
            Ok(answer) => { run.set_answer(Some(answer)); let slug = run.slug.clone(); self.save(&slug); cx.notify(); }
            Err(error) => window.push_notification(Notification::error(format!("Artifact: {error}")), cx),
        }
    }
}

impl Assist {
    pub(super) fn thread_header(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
        let theme = cx.theme().clone();
        let title = self.selected_thread.and_then(|id| self.db.chat_thread(id).ok()).map(|thread| thread.title).unwrap_or_else(|| "Threads".into());
        let weak = cx.entity().downgrade(); let has_thread = self.selected_thread.is_some();
        let new_chat = weak.clone(); let has_problem = self.slug.is_some();
        v_flex().px_3().pb_2().gap_2()
            .child(h_flex().gap_1().min_w_0()
                .child(Button::new("chat-thread-picker").ghost().small().icon(if self.show_threads { IconName::ChevronDown } else { IconName::ChevronRight }).label(title).flex_1().min_w_0()
                    .tooltip("Conversation threads").on_click(cx.listener(|this, _, window, cx| { if this.show_threads { this.show_threads = false; cx.notify(); } else { this.open_thread_list(window, cx); } })))
                .child(Button::new("chat-new-thread").ghost().small().icon(IconName::Plus).tooltip("New thread").accessibility_label("New thread")
                    .dropdown_menu(move |menu, _, _| {
                        let problem = new_chat.clone(); let root = new_chat.clone();
                        menu.item(PopupMenuItem::new("New problem chat").disabled(!has_problem).on_click(move |_, window, cx| { let _ = problem.update(cx, |this, cx| this.new_thread(false, window, cx)); }))
                            .item(PopupMenuItem::new("New Root chat").on_click(move |_, window, cx| { let _ = root.update(cx, |this, cx| this.new_thread(true, window, cx)); }))
                    }))
                .child(Button::new("chat-undo").ghost().small().icon(IconName::Undo2).tooltip("Undo latest branch or deletion").accessibility_label("Undo latest branch or deletion").disabled(!self.db.can_undo_chat())
                    .on_click(cx.listener(|this, _, window, cx| this.undo_thread(window, cx))))
                .child(Button::new("chat-thread-options").ghost().small().icon(IconName::Ellipsis).tooltip("Thread options").disabled(!has_thread)
                    .dropdown_menu(move |menu, _, _| { let weak = weak.clone(); menu.item(PopupMenuItem::new("Delete thread").icon(IconName::Trash).on_click(move |_, window, cx| { let _ = weak.update(cx, |this, cx| this.delete_thread(window, cx)); })) })))
            .when(self.show_threads, |el| el
                .child(v_flex().key_context("ChatThreads")
                    .on_action(cx.listener(|this, _: &NextThread, _, cx| this.navigate_threads(1, cx)))
                    .on_action(cx.listener(|this, _: &PreviousThread, _, cx| this.navigate_threads(-1, cx)))
                    .on_action(cx.listener(|this, _: &OpenThread, window, cx| { if let Some(thread) = this.threads.get(this.thread_selection) { this.select_thread(thread.id, window, cx); } }))
                    .on_action(cx.listener(|this, _: &CloseThreadList, window, cx| { this.show_threads = false; this.composer.focus_handle(cx).focus(window, cx); cx.notify(); }))
                    .child(gpui_kit::component::input::Input::new(&self.thread_search).small()))
                .when(self.threads.is_empty(), |el| el.child(div().px_2().py_2().text_xs().text_color(theme.muted_foreground).child(if self.thread_query.trim().is_empty() { "No threads yet" } else { "No matching chats" })))
                .when(!self.threads.is_empty(), |el| el.child(uniform_list("chat-thread-list", self.threads.len(), cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                    let theme = cx.theme().clone();
                    range.filter_map(|index| this.threads.get(index).map(|thread| {
                        let id = thread.id;
                        h_flex().id(("chat-thread-row", id as u64)).h(px(34.)).w_full().px_2().gap_2().items_center().rounded_md().cursor_pointer()
                            .bg(if index == this.thread_selection { theme.list_active } else { theme.background.opacity(0.) })
                            .hover(|el| el.bg(theme.list_hover))
                            .child(Icon::new(if thread.fork_of.is_some() { IconName::GitBranch } else { IconName::MessageCircle }).xsmall().text_color(theme.muted_foreground))
                            .child(div().flex_1().min_w_0().truncate().text_sm().child(thread.title.clone()))
                            .child(div().text_xs().text_color(theme.muted_foreground).child(if thread.problem.is_none() { "Root" } else { "Problem" }))
                            .on_click(cx.listener(move |this, _, window, cx| this.select_thread(id, window, cx))).into_any_element()
                    })).collect()
                })).track_scroll(&self.thread_scroll).w_full().h(px(self.threads.len().min(5) as f32 * 34.)))))
            .into_any_element()
    }
}
