/// Removes a leading icon prefix, tolerating a missing separating space so that a
/// name typed by hand in the rename prompt still maps to the right note.
pub fn strip_icon<'a>(name: &'a str, icon: &str) -> &'a str {
    match name.strip_prefix(icon) {
        Some(rest) => rest.strip_prefix(' ').unwrap_or(rest),
        None => name,
    }
}

pub fn decorate(clean_name: &str, icon: &str, has_note: bool) -> String {
    if has_note {
        format!("{} {}", icon, clean_name)
    } else {
        clean_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: &str = "📝";

    #[test]
    fn strips_the_icon_and_its_separating_space() {
        assert_eq!(strip_icon("📝 dotfiles", ICON), "dotfiles");
    }

    #[test]
    fn leaves_undecorated_names_alone() {
        assert_eq!(strip_icon("dotfiles", ICON), "dotfiles");
    }

    #[test]
    fn strips_an_icon_written_without_a_space() {
        assert_eq!(strip_icon("📝dotfiles", ICON), "dotfiles");
    }

    #[test]
    fn a_bare_icon_strips_to_the_empty_string() {
        assert_eq!(strip_icon("📝", ICON), "");
    }

    #[test]
    fn does_not_strip_an_icon_that_only_appears_later() {
        assert_eq!(strip_icon("dotfiles 📝", ICON), "dotfiles 📝");
    }

    #[test]
    fn decorates_only_when_a_note_exists() {
        assert_eq!(decorate("dotfiles", ICON, true), "📝 dotfiles");
        assert_eq!(decorate("dotfiles", ICON, false), "dotfiles");
    }

    #[test]
    fn decorate_and_strip_round_trip() {
        for has_note in [true, false] {
            let decorated = decorate("dotfiles", ICON, has_note);
            assert_eq!(strip_icon(&decorated, ICON), "dotfiles");
        }
    }
}
