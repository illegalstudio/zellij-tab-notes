use std::collections::BTreeSet;

/// Parses the stdout of `find <dir> -maxdepth 1 -name '*.md' -size +0c` into the set of
/// clean tab names that currently have a note.
pub fn parse_note_listing(stdout: &str) -> BTreeSet<String> {
    stdout
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.rsplit('/').next())
        .filter_map(|file| file.strip_suffix(".md"))
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_note_names_from_find_output() {
        let stdout = "/notes/dotfiles/main.md\n/notes/dotfiles/feature-login.md\n";
        let parsed = parse_note_listing(stdout);
        assert!(parsed.contains("main"));
        assert!(parsed.contains("feature-login"));
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn handles_names_containing_spaces_and_dots() {
        let stdout = "/notes/s/my project.md\n/notes/s/v1.2 notes.md\n";
        let parsed = parse_note_listing(stdout);
        assert!(parsed.contains("my project"));
        assert!(parsed.contains("v1.2 notes"));
    }

    #[test]
    fn ignores_blank_lines_and_non_markdown_entries() {
        let stdout = "\n/notes/s/a.md\n/notes/s/README\n\n";
        assert_eq!(parse_note_listing(stdout).len(), 1);
    }

    #[test]
    fn empty_output_is_an_empty_set() {
        assert!(parse_note_listing("").is_empty());
    }
}
