use std::path::{Path, PathBuf};

pub const MAX_NAME_LEN: usize = 200;

/// Turns a tab or session name into something usable as a single path component.
///
/// This is not a security boundary — `run_command` runs without a shell, so quoting
/// and injection are not a concern. It only guarantees the result is one path
/// component that cannot escape its parent directory.
///
/// The result is the **note key**: the identity a note file is stored under. It is
/// deliberately *not* idempotent (see `sanitization_is_deliberately_not_idempotent`),
/// so never apply it to a value that is already a key — use [`note_path_from_key`].
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
    match truncated.as_str() {
        "" | "." | ".." => "_".to_string(),
        other => other.to_string(),
    }
}

pub fn session_dir(notes_dir: &Path, session: &str) -> PathBuf {
    notes_dir.join(sanitize_tab_name(session))
}

/// Builds the note path for a raw tab name, sanitizing it into a key on the way.
pub fn note_path(notes_dir: &Path, session: &str, tab: &str) -> PathBuf {
    note_path_from_key(notes_dir, session, &sanitize_tab_name(tab))
}

/// Builds the note path for a value that is *already* a note key.
///
/// Sanitizing twice is not a no-op — truncation can expose a trailing space that a
/// second pass would trim — so a caller holding a key must never route it back
/// through [`note_path`].
pub fn note_path_from_key(notes_dir: &Path, session: &str, key: &str) -> PathBuf {
    session_dir(notes_dir, session).join(format!("{key}.md"))
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

    #[test]
    fn maintains_max_length_with_trailing_whitespace() {
        let input = "x".repeat(199) + " " + &"y".repeat(300);
        let result = sanitize_tab_name(&input);
        assert_eq!(result.chars().count(), MAX_NAME_LEN);
    }

    #[test]
    fn distinguishes_dot_with_padding_from_dot_alone() {
        let input = ".".to_string() + &" ".repeat(199) + &"z".repeat(50);
        let result = sanitize_tab_name(&input);
        assert_ne!(result, sanitize_tab_name("."));
    }

    #[test]
    fn turns_control_characters_into_spaces() {
        assert_eq!(sanitize_tab_name("a\tb"), "a b");
        assert_eq!(sanitize_tab_name("a\nb"), "a b");
        assert_eq!(sanitize_tab_name("a\u{7}b"), "a b");
        // Control characters at the edges become spaces and are then trimmed away.
        assert_eq!(sanitize_tab_name("\ta\n"), "a");
    }

    #[test]
    fn sanitization_is_deliberately_not_idempotent() {
        // INTENTIONAL, DO NOT "FIX". Truncation can leave a trailing space that a
        // second pass would trim. Trimming after truncation was tried and removed:
        // it collapsed distinct long names onto the same key (and onto `_`), letting
        // two tabs silently share one note file. The cost is that
        // `sanitize_tab_name` must never be applied to a value that is already a
        // key — which is precisely why `note_path_from_key` exists.
        let input = "x".repeat(199) + " " + &"y".repeat(300);
        let once = sanitize_tab_name(&input);
        let twice = sanitize_tab_name(&once);
        assert_eq!(once.chars().count(), MAX_NAME_LEN);
        assert_ne!(once, twice, "double sanitization must be assumed to change the key");
    }

    #[test]
    fn builds_a_note_path_from_a_key_without_sanitizing_again() {
        let dir = Path::new("/notes");
        let key = sanitize_tab_name("feature/login");
        assert_eq!(
            note_path_from_key(dir, "dotfiles", &key),
            note_path(dir, "dotfiles", "feature/login")
        );
        // A key that would change under a second pass keeps its exact spelling.
        assert_eq!(
            note_path_from_key(dir, "dotfiles", "trailing "),
            PathBuf::from("/notes/dotfiles/trailing .md")
        );
    }
}
