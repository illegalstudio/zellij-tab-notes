use crate::icon::{decorate, strip_icon};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabView {
    pub id: usize,
    pub position: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Rename tab at `position` so its name matches whether it has a note.
    RenameTab { position: u32, name: String },
    /// Move a note file because its tab was renamed. Both values are clean tab names.
    MoveNote { from: String, to: String },
}

/// Keeps tab names in sync with the set of tabs that have notes.
///
/// `reconcile` is idempotent: feeding it a settled state produces no actions, which is
/// what stops `rename_tab` from re-triggering itself through the resulting `TabUpdate`.
pub struct Reconciler {
    icon: String,
    /// tab id -> last seen clean name. The id is stable across MoveTab and across the
    /// closing of other tabs, which is what makes rename detection trustworthy.
    known: BTreeMap<usize, String>,
}

impl Reconciler {
    pub fn new(icon: impl Into<String>) -> Self {
        Self { icon: icon.into(), known: BTreeMap::new() }
    }

    pub fn reconcile(&mut self, tabs: &[TabView], notes: &mut BTreeSet<String>) -> Vec<Action> {
        let mut actions = Vec::new();

        for tab in tabs {
            let clean = strip_icon(&tab.name, &self.icon).to_string();

            if let Some(previous) = self.known.get(&tab.id) {
                if previous != &clean && notes.contains(previous) && !notes.contains(&clean) {
                    notes.remove(previous);
                    notes.insert(clean.clone());
                    actions.push(Action::MoveNote { from: previous.clone(), to: clean.clone() });
                }
            }
            self.known.insert(tab.id, clean.clone());

            let expected = decorate(&clean, &self.icon, notes.contains(&clean));
            if expected != tab.name {
                actions.push(Action::RenameTab { position: tab.position as u32, name: expected });
            }
        }

        self.known.retain(|id, _| tabs.iter().any(|tab| tab.id == *id));
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: &str = "📝";

    fn tab(id: usize, position: usize, name: &str) -> TabView {
        TabView { id, position, name: name.to_string() }
    }

    fn notes(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn adds_the_icon_to_a_tab_that_has_a_note() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&["dotfiles"]);
        let actions = r.reconcile(&[tab(1, 0, "dotfiles")], &mut n);
        assert_eq!(
            actions,
            vec![Action::RenameTab { position: 0, name: "📝 dotfiles".to_string() }]
        );
    }

    #[test]
    fn leaves_a_tab_without_a_note_alone() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&[]);
        assert_eq!(r.reconcile(&[tab(1, 0, "dotfiles")], &mut n), vec![]);
    }

    #[test]
    fn removes_a_stale_icon_when_the_note_is_gone() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&[]);
        let actions = r.reconcile(&[tab(1, 0, "📝 dotfiles")], &mut n);
        assert_eq!(
            actions,
            vec![Action::RenameTab { position: 0, name: "dotfiles".to_string() }]
        );
    }

    #[test]
    fn is_idempotent() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&["dotfiles"]);
        let first = r.reconcile(&[tab(1, 0, "dotfiles")], &mut n);
        assert_eq!(first.len(), 1);
        // Zellij has now applied the rename; the tab comes back decorated.
        let second = r.reconcile(&[tab(1, 0, "📝 dotfiles")], &mut n);
        assert_eq!(second, vec![], "a settled state must produce no actions");
        let third = r.reconcile(&[tab(1, 0, "📝 dotfiles")], &mut n);
        assert_eq!(third, vec![]);
    }

    #[test]
    fn follows_a_rename_by_tab_id() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&["old"]);
        r.reconcile(&[tab(7, 0, "old")], &mut n);
        let actions = r.reconcile(&[tab(7, 0, "new")], &mut n);
        assert_eq!(
            actions,
            vec![
                Action::MoveNote { from: "old".to_string(), to: "new".to_string() },
                Action::RenameTab { position: 0, name: "📝 new".to_string() },
            ]
        );
        assert_eq!(n, notes(&["new"]), "the note set must be updated in place");
    }

    #[test]
    fn follows_a_rename_that_kept_the_icon_in_the_typed_name() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&["old"]);
        r.reconcile(&[tab(7, 0, "old")], &mut n);
        let actions = r.reconcile(&[tab(7, 0, "📝 new")], &mut n);
        assert_eq!(
            actions,
            vec![Action::MoveNote { from: "old".to_string(), to: "new".to_string() }],
            "the name is already correct, only the file moves"
        );
    }

    #[test]
    fn does_not_move_a_note_for_a_tab_that_never_had_one() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&[]);
        r.reconcile(&[tab(7, 0, "old")], &mut n);
        assert_eq!(r.reconcile(&[tab(7, 0, "new")], &mut n), vec![]);
    }

    #[test]
    fn moving_a_tab_does_not_look_like_a_rename() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&["a"]);
        r.reconcile(&[tab(1, 0, "a"), tab(2, 1, "b")], &mut n);
        // MoveTab swaps positions but leaves tab ids untouched.
        let actions = r.reconcile(&[tab(2, 0, "b"), tab(1, 1, "📝 a")], &mut n);
        assert_eq!(actions, vec![], "positions changed, names did not");
    }

    #[test]
    fn forgets_closed_tabs() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&["a"]);
        r.reconcile(&[tab(1, 0, "a")], &mut n);
        r.reconcile(&[], &mut n);
        // Tab id 1 is reused by a brand new tab named "b": no note must be moved.
        assert_eq!(r.reconcile(&[tab(1, 0, "b")], &mut n), vec![]);
    }

    #[test]
    fn does_not_move_a_note_onto_another_open_tabs_note() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&["old", "y"]);
        // Settle with two tabs and two notes
        r.reconcile(&[tab(1, 0, "old"), tab(2, 1, "y")], &mut n);
        // Rename tab 1 from "old" to "y" — destination has a note
        let actions = r.reconcile(&[tab(1, 0, "y"), tab(2, 1, "📝 y")], &mut n);
        // Should not emit MoveNote, but may emit RenameTab
        let has_move_note = actions.iter().any(|a| matches!(a, Action::MoveNote { .. }));
        assert!(!has_move_note, "must not emit MoveNote when destination has a note");
        // Note set must be unchanged
        assert_eq!(n, notes(&["old", "y"]), "both notes must survive");
    }

    #[test]
    fn does_not_move_a_note_onto_an_orphan_note_file() {
        let mut r = Reconciler::new(ICON);
        let mut n = notes(&["old", "z"]);
        // Settle with one tab; "z" has no tab
        r.reconcile(&[tab(1, 0, "old")], &mut n);
        // Rename tab 1 from "old" to "z" — destination has an orphan note
        let actions = r.reconcile(&[tab(1, 0, "z")], &mut n);
        // Should not emit MoveNote
        let has_move_note = actions.iter().any(|a| matches!(a, Action::MoveNote { .. }));
        assert!(!has_move_note, "must not emit MoveNote when destination has an orphan note");
        // Note set must be unchanged
        assert_eq!(n, notes(&["old", "z"]), "both notes must survive");
    }
}
