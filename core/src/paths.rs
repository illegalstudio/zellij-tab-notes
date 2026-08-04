use std::path::{Path, PathBuf};

pub const MAX_NAME_LEN: usize = 200;

/// Turns a tab or session name into something usable as a single path component.
///
/// This is not a security boundary — `run_command` runs without a shell, so quoting
/// and injection are not a concern. It only guarantees the result is one path
/// component that cannot escape its parent directory.
pub fn sanitize_tab_name(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let truncated: String = replaced.trim().chars().take(MAX_NAME_LEN).collect();
    match truncated.trim() {
        "" | "." | ".." => "_".to_string(),
        other => other.to_string(),
    }
}

pub fn session_dir(notes_dir: &Path, session: &str) -> PathBuf {
    notes_dir.join(sanitize_tab_name(session))
}

pub fn note_path(notes_dir: &Path, session: &str, tab: &str) -> PathBuf {
    session_dir(notes_dir, session).join(format!("{}.md", sanitize_tab_name(tab)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_ordinary_names_untouched() {
        assert_eq!(sanitize_tab_name("dotfiles"), "dotfiles");
        assert_eq!(sanitize_tab_name("my project"), "my project");
    }

    #[test]
    fn replaces_path_separators() {
        assert_eq!(sanitize_tab_name("feature/login"), "feature-login");
        assert_eq!(sanitize_tab_name("a\\b"), "a-b");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(sanitize_tab_name("  dotfiles  "), "dotfiles");
    }

    #[test]
    fn replaces_names_that_would_escape_the_directory() {
        assert_eq!(sanitize_tab_name("."), "_");
        assert_eq!(sanitize_tab_name(".."), "_");
        assert_eq!(sanitize_tab_name("   "), "_");
        assert_eq!(sanitize_tab_name(""), "_");
    }

    #[test]
    fn truncates_on_a_character_boundary() {
        let long = "à".repeat(300);
        let result = sanitize_tab_name(&long);
        assert_eq!(result.chars().count(), MAX_NAME_LEN);
    }

    #[test]
    fn builds_a_note_path_under_the_session_directory() {
        let dir = Path::new("/notes");
        assert_eq!(
            note_path(dir, "dotfiles", "feature/login"),
            PathBuf::from("/notes/dotfiles/feature-login.md")
        );
    }

    #[test]
    fn builds_a_session_directory() {
        assert_eq!(
            session_dir(Path::new("/notes"), "dotfiles"),
            PathBuf::from("/notes/dotfiles")
        );
    }
}
