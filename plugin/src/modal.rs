use crate::fs_ops;
use tab_notes_core::config::Config;
use tab_notes_core::icon::strip_icon;
use tab_notes_core::paths::note_path;
use tab_notes_core::viewport::{clamp_scroll, is_heading, wrap};
use zellij_tile::prelude::*;

pub struct Modal {
    config: Result<Config, String>,
    session: Option<String>,
    tab: Option<String>,
    content: Option<String>,
    status: Option<String>,
    scroll: usize,
    watcher_id: Option<u32>,
    confirming_delete: bool,
}

impl Modal {
    pub fn new(config: Result<Config, String>) -> Self {
        Self {
            config,
            session: None,
            tab: None,
            content: None,
            status: None,
            scroll: 0,
            watcher_id: None,
            confirming_delete: false,
        }
    }

    pub fn load(&mut self) {
        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::TabUpdate,
            EventType::RunCommandResult,
            EventType::Key,
        ]);
    }

    pub fn update(&mut self, event: Event) -> bool {
        let Ok(config) = self.config.clone() else {
            return false;
        };
        match event {
            Event::SessionUpdate(sessions, _) => {
                let Some(current) = sessions.iter().find(|s| s.is_current_session) else {
                    return false;
                };
                if self.session.as_deref() != Some(current.name.as_str()) {
                    self.session = Some(current.name.clone());
                    // The watcher is addressed by plugin id, discovered from the session's
                    // plugin list: no need to duplicate its URL in the modal's configuration.
                    self.watcher_id = current
                        .plugins
                        .iter()
                        .find(|(_, info)| {
                            info.configuration.get("role").map(String::as_str) != Some("modal")
                                && info.location.contains("tab-notes")
                        })
                        .map(|(id, _)| *id);
                    // Note-scoped state must not survive a change of which note is
                    // shown. `content` included: while the new read is in flight it
                    // would otherwise answer `has_note()` about the previous note
                    // while `delete_note()` already targets the new one.
                    self.confirming_delete = false;
                    self.status = None;
                    self.content = None;
                    self.read_note();
                }
                true
            }
            Event::TabUpdate(tabs) => {
                let Some(active) = tabs.iter().find(|tab| tab.active) else {
                    return false;
                };
                let clean = strip_icon(&active.name, &config.icon).to_string();
                if self.tab.as_deref() != Some(clean.as_str()) {
                    self.tab = Some(clean);
                    self.scroll = 0;
                    // Note-scoped state must not survive a change of which note is
                    // shown. `content` included: while the new read is in flight it
                    // would otherwise answer `has_note()` about the previous tab's
                    // note while `delete_note()` already targets the new one.
                    self.confirming_delete = false;
                    self.status = None;
                    self.content = None;
                    self.read_note();
                }
                true
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                match fs_ops::op_of(&context) {
                    Some(fs_ops::OP_READ) => {
                        self.content = if exit_code == Some(0) {
                            Some(String::from_utf8_lossy(&stdout).to_string())
                        } else {
                            None
                        };
                        true
                    }
                    // Report what actually happened, and only tell the watcher the
                    // note is gone once the `rm` has really finished — sending the
                    // pipe from `delete_note` raced the subprocess, so the refresh
                    // could list the note that was still there and leave the icon on.
                    Some(fs_ops::OP_DELETE) => {
                        if exit_code == Some(0) {
                            self.content = None;
                            self.status = Some("note deleted".to_string());
                            self.send_to_watcher("notes-changed", None);
                        } else {
                            eprintln!(
                                "tab-notes: delete failed: {}",
                                String::from_utf8_lossy(&stderr)
                            );
                            self.status = Some("delete failed — see the Zellij log".to_string());
                        }
                        true
                    }
                    _ => false,
                }
            }
            Event::Key(key) => self.on_key(key),
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.config = Err(
                    "tab-notes: permissions denied — close this pane, reload the plugin \
                     and accept the request"
                        .to_string(),
                );
                true
            }
            _ => false,
        }
    }

    fn read_note(&mut self) {
        let (Ok(config), Some(session), Some(tab)) = (
            self.config.as_ref(),
            self.session.as_ref(),
            self.tab.as_ref(),
        ) else {
            return;
        };
        fs_ops::read_note(&note_path(&config.notes_dir, session, tab));
    }

    fn has_note(&self) -> bool {
        self.content.as_ref().is_some_and(|c| !c.trim().is_empty())
    }

    fn on_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Esc | BareKey::Char('q') => {
                close_self();
                false
            }
            BareKey::Char('j') | BareKey::Down => {
                self.scroll = self.scroll.saturating_add(1);
                true
            }
            BareKey::Char('k') | BareKey::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                true
            }
            BareKey::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                true
            }
            BareKey::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                true
            }
            // Without a tab name there is nothing to open: the watcher drops a
            // payload-less `edit-note` silently, so the modal would just close.
            BareKey::Char('e') if self.tab.is_some() => {
                self.send_to_watcher("edit-note", self.tab.clone());
                close_self();
                false
            }
            BareKey::Char('d') if self.has_note() && !self.confirming_delete => {
                self.confirming_delete = true;
                true
            }
            BareKey::Char('y') if self.confirming_delete => {
                self.confirming_delete = false;
                self.delete_note();
                true
            }
            BareKey::Char('n') if self.confirming_delete => {
                self.confirming_delete = false;
                true
            }
            _ => false,
        }
    }

    fn send_to_watcher(&self, name: &str, payload: Option<String>) {
        let Some(watcher_id) = self.watcher_id else {
            eprintln!("tab-notes: no watcher instance found, is it in load_plugins?");
            return;
        };
        let mut message = MessageToPlugin::new(name).with_destination_plugin_id(watcher_id);
        if let Some(payload) = payload {
            message = message.with_payload(payload);
        }
        pipe_message_to_plugin(message);
    }

    fn delete_note(&mut self) {
        let (Ok(config), Some(session), Some(tab)) = (
            self.config.as_ref(),
            self.session.as_ref(),
            self.tab.as_ref(),
        ) else {
            return;
        };
        // The modal performs its own destructive operation so that deleting still works
        // when no watcher is loaded; the watcher is only told to refresh the icon.
        // Everything the user is told about the outcome happens in the OP_DELETE arm
        // of `update`, once the command has actually reported an exit code.
        fs_ops::delete_note(&note_path(&config.notes_dir, session, tab));
        self.status = Some("deleting…".to_string());
    }

    pub fn render(&mut self, rows: usize, cols: usize) {
        if let Err(error) = &self.config {
            print_text_with_coordinates(Text::new(error), 0, 0, Some(cols), None);
            return;
        }
        let title = match &self.tab {
            Some(tab) => format!("Note · {tab}"),
            None => "Note".to_string(),
        };
        print_text_with_coordinates(Text::new(&title).color_range(2, ..), 0, 0, Some(cols), None);

        let body_rows = rows.saturating_sub(3);
        match &self.content {
            Some(content) if self.has_note() => {
                let lines = wrap(content, cols);
                self.scroll = clamp_scroll(self.scroll, lines.len(), body_rows);
                for (row, line) in lines.iter().skip(self.scroll).take(body_rows).enumerate() {
                    let text = if is_heading(line) {
                        Text::new(line).color_range(0, ..)
                    } else {
                        Text::new(line)
                    };
                    print_text_with_coordinates(text, 0, row + 2, Some(cols), None);
                }
            }
            _ => {
                let empty = match &self.tab {
                    Some(tab) => format!("No note for «{tab}» — press e to create one"),
                    None => "Loading…".to_string(),
                };
                print_text_with_coordinates(Text::new(empty).dim_all(), 0, 2, Some(cols), None);
            }
        }

        let footer = match (&self.status, self.confirming_delete) {
            (_, true) => "delete this note? y/n".to_string(),
            (Some(status), _) => status.clone(),
            _ => "e edit · d delete · j/k scroll · Esc close".to_string(),
        };
        print_text_with_coordinates(
            Text::new(footer).dim_all(),
            0,
            rows.saturating_sub(1),
            Some(cols),
            None,
        );
    }
}
