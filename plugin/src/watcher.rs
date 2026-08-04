use tab_notes_core::config::Config;
use zellij_tile::prelude::*;

pub struct Watcher {
    config: Result<Config, String>,
}

impl Watcher {
    pub fn new(config: Result<Config, String>) -> Self {
        Self { config }
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

    pub fn update(&mut self, _event: Event) -> bool {
        false
    }

    pub fn pipe(&mut self, _pipe_message: PipeMessage) -> bool {
        false
    }
}
