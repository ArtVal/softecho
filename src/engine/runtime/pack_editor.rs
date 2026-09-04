//! Редактор пользовательского набора (состояние + команды Engine).

use super::super::data::{
    clone_pack_to_user, is_user_pack, load_editable_pack, save_user_pack, EditablePack,
};
use super::super::exercise::{Exercise, ExerciseStage};
use super::super::protocol::Screen;
use super::Engine;

pub struct PackEditorState {
    pub pack_id: String,
    pub draft: EditablePack,
    pub error: Option<String>,
    pub note: Option<String>,
    /// Есть правки после последнего сохранения.
    pub dirty: bool,
}

impl Engine {
    pub(super) fn open_pack_editor(&mut self) {
        self.abort_listen();
        self.session = None;
        let id = self.pack_id().to_string();
        if !is_user_pack(&id) {
            self.load_error = Some(
                "Встроенный набор нельзя менять. Нажмите «Сделать копию» — правка будет в ваших данных."
                    .into(),
            );
            self.screen = Screen::PackEditor;
            self.pack_editor = None;
            return;
        }
        match load_editable_pack(&id) {
            Ok(draft) => {
                self.load_error = None;
                self.pack_editor = Some(PackEditorState {
                    pack_id: id,
                    draft,
                    error: None,
                    note: None,
                    dirty: false,
                });
                self.screen = Screen::PackEditor;
            }
            Err(e) => {
                self.load_error = Some(e);
                self.pack_editor = None;
                self.screen = Screen::Home;
            }
        }
    }

    pub(super) fn clone_pack_for_edit(&mut self) {
        self.abort_listen();
        self.session = None;
        let source = self.pack_id().to_string();
        match clone_pack_to_user(&source, "") {
            Ok((id, draft)) => {
                self.pack = draft.to_active_pack();
                self.progress.set_pack(&id);
                self.persist_progress();
                self.load_error = None;
                self.pack_editor = Some(PackEditorState {
                    pack_id: id,
                    draft,
                    error: None,
                    note: Some("Копия сохранена. Можно править и сохранять.".into()),
                    dirty: false,
                });
                self.screen = Screen::PackEditor;
            }
            Err(e) => {
                self.load_error = Some(e);
            }
        }
    }

    pub(super) fn editor_disable(&mut self, index: usize) {
        let Some(ed) = self.pack_editor.as_mut() else {
            return;
        };
        if index >= ed.draft.exercises.len() {
            return;
        }
        if ed.draft.exercises.len() == 1 {
            ed.error = Some("Нельзя отключить последнее активное задание.".into());
            return;
        }
        let ex = ed.draft.exercises.remove(index);
        ed.draft.disabled.push(ex);
        ed.error = None;
        ed.note = None;
        ed.dirty = true;
    }

    pub(super) fn editor_enable(&mut self, index: usize) {
        let Some(ed) = self.pack_editor.as_mut() else {
            return;
        };
        if index >= ed.draft.disabled.len() {
            return;
        }
        let ex = ed.draft.disabled.remove(index);
        ed.draft.exercises.push(ex);
        ed.error = None;
        ed.note = None;
        ed.dirty = true;
    }

    pub(super) fn editor_add_read_aloud(
        &mut self,
        prompt: String,
        text: String,
        stage: ExerciseStage,
    ) {
        let Some(ed) = self.pack_editor.as_mut() else {
            return;
        };
        let prompt = if prompt.trim().is_empty() {
            "Скажите".into()
        } else {
            prompt.trim().to_string()
        };
        let text = text.trim().to_string();
        if text.is_empty() {
            ed.error = Some("Введите текст задания.".into());
            return;
        }
        ed.draft.exercises.push(Exercise::ReadAloud {
            stage: Some(stage),
            prompt,
            text,
            speak: None,
            image: None,
        });
        ed.error = None;
        ed.note = Some("Задание добавлено — нажмите «Сохранить».".into());
        ed.dirty = true;
    }

    pub(super) fn editor_save(&mut self) {
        let Some(ed) = self.pack_editor.as_ref() else {
            return;
        };
        let id = ed.pack_id.clone();
        let draft = ed.draft.clone();
        match save_user_pack(&id, &draft) {
            Ok(_) => {
                self.pack = draft.to_active_pack();
                if let Some(ed) = self.pack_editor.as_mut() {
                    ed.error = None;
                    ed.note = Some("Сохранено.".into());
                    ed.draft = draft;
                    ed.dirty = false;
                }
                self.load_error = None;
            }
            Err(e) => {
                if let Some(ed) = self.pack_editor.as_mut() {
                    ed.error = Some(e);
                    ed.note = None;
                }
            }
        }
    }

    /// Уход из редактора. `discard` — бросить несохранённые правки.
    pub(super) fn leave_pack_editor(&mut self, discard: bool) {
        if let Some(ed) = self.pack_editor.as_mut() {
            if ed.dirty && !discard {
                ed.error = Some(
                    "Есть несохранённые изменения. Сохраните или нажмите «Уйти без сохранения»."
                        .into(),
                );
                ed.note = None;
                return;
            }
        }
        self.pack_editor = None;
        self.screen = Screen::Home;
    }
}
