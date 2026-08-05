use crate::fs_ops;
use tab_notes_core::config::Config;
use tab_notes_core::icon::strip_icon;
use tab_notes_core::markdown::{self, LineKind, RenderedLine, SpanKind};
use tab_notes_core::paths::note_path;
use tab_notes_core::viewport::clamp_scroll;
use zellij_tile::prelude::*;

pub struct Modal {
    config: Result<Config, String>,
    session: Option<String>,
    tab: Option<String>,
    content: Option<String>,
    status: Option<String>,
    scroll: usize,
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
            confirming_delete: false,
        }
    }

    pub fn load(&mut self) {
        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::TabUpdate,
            EventType::RunCommandResult,
            EventType::EditPaneExited,
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
                            Self::send_to_watcher("tab-notes:notes-changed", None);
                        } else {
                            eprintln!(
                                "tab-notes: delete failed: {}",
                                String::from_utf8_lossy(&stderr)
                            );
                            self.status = Some("delete failed — see the Zellij log".to_string());
                        }
                        true
                    }
                    // The post-edit cleanup finished: re-read so the preview shows what
                    // was just written, and tell the watcher in case the note came into
                    // existence or stopped existing.
                    Some(fs_ops::OP_CLEANUP) => {
                        self.read_note();
                        Self::send_to_watcher("tab-notes:notes-changed", None);
                        true
                    }
                    _ => false,
                }
            }
            // Only ever delivered for a pane this plugin opened, but the op tag keeps that
            // true even if the modal grows another kind of pane later.
            Event::EditPaneExited(_pane_id, _exit_code, context) => {
                if fs_ops::op_of(&context) != Some(fs_ops::OP_EDIT) {
                    return false;
                }
                let (Some(session), Some(tab)) = (self.session.as_ref(), self.tab.as_ref()) else {
                    return false;
                };
                // An aborted edit leaves a zero-byte file behind; remove it so it never
                // counts as a note. Re-reading is chained to that command's result.
                fs_ops::delete_if_empty(&note_path(&config.notes_dir, session, tab));
                self.status = None;
                true
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

    fn open_editor(&mut self) {
        let (Ok(config), Some(session), Some(tab)) = (
            self.config.as_ref(),
            self.session.as_ref(),
            self.tab.as_ref(),
        ) else {
            return;
        };
        open_file_floating(
            FileToOpen::new(note_path(&config.notes_dir, session, tab)),
            None,
            fs_ops::context_with_tab(fs_ops::OP_EDIT, tab),
        );
        self.status = Some("editing in $EDITOR…".to_string());
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
            // The modal opens the editor itself and stays alive behind it. Zellij delivers
            // `EditPaneExited` only to the plugin that opened the file, and that event is
            // what lets the modal show the edited note instead of disappearing.
            BareKey::Char('e') if self.tab.is_some() => {
                self.open_editor();
                true
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

    /// Broadcasts to every plugin in the session, which is how the background watcher is
    /// reached.
    ///
    /// The watcher's plugin id cannot be discovered: `SessionInfo.plugins` is empty in the
    /// `SessionUpdate` delivered to plugins, so an id-addressed message had nowhere to go and
    /// the modal closed doing nothing. A `MessageToPlugin` carrying neither a url nor a
    /// destination id is routed to all plugin ids, headless ones included. Other plugins
    /// ignore a pipe name they do not know, and the modal ignores pipes entirely, so only the
    /// watcher acts on it. Names are prefixed because every plugin now sees them.
    fn send_to_watcher(name: &str, payload: Option<String>) {
        let mut message = MessageToPlugin::new(name);
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
                let lines = markdown::wrap(&markdown::render(content), cols);
                self.scroll = clamp_scroll(self.scroll, lines.len(), body_rows);
                for (row, line) in lines.iter().skip(self.scroll).take(body_rows).enumerate() {
                    print_text_with_coordinates(style(line, cols), 0, row + 2, Some(cols), None);
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

/// Maps a line's meaning onto Zellij's styling primitives.
///
/// A terminal has one font size, so headings read as headings through case, colour and
/// the rule underneath them, not through size. Note that `Text` is bold by default —
/// there is no `bold_range`, only `unbold_*` — so body text is explicitly unbolded and
/// weight is what sets headings and strong runs apart.
fn style(line: &RenderedLine, cols: usize) -> Text {
    match line.kind {
        LineKind::Rule => Text::new("─".repeat(cols)).dim_all(),
        LineKind::Heading1 => Text::new(&line.text).color_range(0, ..),
        LineKind::Heading2 => Text::new(&line.text).color_range(3, ..),
        LineKind::Heading3 => Text::new(&line.text).color_range(1, ..),
        // A finished task is still readable but stops competing for attention.
        LineKind::Checkbox { done: true } => body(line).dim_all(),
        LineKind::Checkbox { done: false } => body(line).color_range(2, 0..1),
        LineKind::Bullet => body(line).color_range(1, 0..1),
        LineKind::Quote => body(line).dim_all(),
        LineKind::Code => Text::new(&line.text).color_range(1, ..).unbold_all(),
        LineKind::Plain => body(line),
    }
}

/// Unbolds everything except the strong runs, and colours the inline code runs.
fn body(line: &RenderedLine) -> Text {
    let mut text = Text::new(&line.text);
    let len = line.text.chars().count();
    let strong: Vec<(usize, usize)> = line
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::Strong)
        .map(|s| (s.start, s.end))
        .collect();

    if strong.is_empty() {
        text = text.unbold_all();
    } else {
        // There is no way to add weight, only to remove it, so the strong runs are the
        // gaps left between the ranges we unbold.
        let mut cursor = 0;
        for (start, end) in &strong {
            if cursor < *start {
                text = text.unbold_range(cursor..*start);
            }
            cursor = *end;
        }
        if cursor < len {
            text = text.unbold_range(cursor..len);
        }
    }

    for span in line.spans.iter().filter(|s| s.kind == SpanKind::Code) {
        text = text.color_range(1, span.start..span.end);
    }
    text
}
