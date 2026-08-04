use tab_notes_core::config::Config;
use zellij_tile::prelude::*;

pub struct Modal {
    config: Result<Config, String>,
}

impl Modal {
    pub fn new(config: Result<Config, String>) -> Self {
        Self { config }
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

    pub fn update(&mut self, _event: Event) -> bool {
        false
    }

    pub fn render(&mut self, _rows: usize, _cols: usize) {
        if let Err(error) = &self.config {
            print_text_with_coordinates(Text::new(error), 0, 0, None, None);
        }
    }
}
