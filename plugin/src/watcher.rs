use tab_notes_core::config::Config;
use crate::fs_ops;
use std::collections::BTreeSet;
use tab_notes_core::listing::parse_note_listing;
use tab_notes_core::paths::{note_path, session_dir};
use tab_notes_core::reconcile::{Action, Reconciler, TabView};
use zellij_tile::prelude::*;

pub struct Watcher {
    config: Result<Config, String>,
    reconciler: Option<Reconciler>,
    session: Option<String>,
    tabs: Vec<TabView>,
    notes: BTreeSet<String>,
}

impl Watcher {
    pub fn new(config: Result<Config, String>) -> Self {
        let reconciler = config.as_ref().ok().map(|c| Reconciler::new(c.icon.clone()));
        Self {
            config,
            reconciler,
            session: None,
            tabs: Vec::new(),
            notes: BTreeSet::new(),
        }
    }

    pub fn load(&mut self) {
        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::TabUpdate,
            EventType::RunCommandResult,
            EventType::EditPaneExited,
        ]);
    }

    pub fn update(&mut self, event: Event) -> bool {
        let Ok(config) = self.config.clone() else {
            return false;
        };
        match event {
            Event::SessionUpdate(sessions, _) => {
                if let Some(current) = sessions.iter().find(|s| s.is_current_session) {
                    if self.session.as_deref() != Some(current.name.as_str()) {
                        self.session = Some(current.name.clone());
                        fs_ops::ensure_dir(&session_dir(&config.notes_dir, &current.name));
                    }
                }
            }
            Event::TabUpdate(tabs) => {
                self.tabs = tabs
                    .iter()
                    .map(|tab| TabView {
                        id: tab.tab_id,
                        position: tab.position,
                        name: tab.name.clone(),
                    })
                    .collect();
                self.apply();
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                self.on_command_result(exit_code, stdout, stderr, context);
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                // Reuse the "not configured" inert path rather than adding a second flag:
                // every guard in this struct already checks it.
                eprintln!("tab-notes: permissions denied, the watcher will stay inert");
                self.config = Err("tab-notes: permissions denied".to_string());
            }
            _ => {}
        }
        false
    }

    fn on_command_result(
        &mut self,
        exit_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        context: std::collections::BTreeMap<String, String>,
    ) {
        match fs_ops::op_of(&context) {
            Some(fs_ops::OP_ENSURE_DIR) => self.refresh(),
            Some(fs_ops::OP_LIST) => {
                // A non-zero exit means the directory does not exist yet: no notes.
                self.notes = if exit_code == Some(0) {
                    parse_note_listing(&String::from_utf8_lossy(&stdout))
                } else {
                    eprintln!(
                        "tab-notes: listing failed: {}",
                        String::from_utf8_lossy(&stderr)
                    );
                    BTreeSet::new()
                };
                self.apply();
            }
            _ => {}
        }
    }

    /// Re-reads the notes directory. Everything downstream flows from the result.
    pub fn refresh(&mut self) {
        let (Ok(config), Some(session)) = (self.config.as_ref(), self.session.as_ref()) else {
            return;
        };
        fs_ops::list_notes(&session_dir(&config.notes_dir, session));
    }

    fn apply(&mut self) {
        let (Ok(config), Some(session), Some(reconciler)) = (
            self.config.as_ref(),
            self.session.as_ref(),
            self.reconciler.as_mut(),
        ) else {
            return;
        };
        for action in reconciler.reconcile(&self.tabs, &mut self.notes) {
            match action {
                Action::RenameTab { position, name } => rename_tab(position, &name),
                Action::MoveNote { from, to } => fs_ops::move_note(
                    &note_path(&config.notes_dir, session, &from),
                    &note_path(&config.notes_dir, session, &to),
                ),
            }
        }
    }

    pub fn pipe(&mut self, _pipe_message: PipeMessage) -> bool {
        false
    }
}
