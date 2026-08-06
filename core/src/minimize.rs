//! When the modal should grow back after being minimised.
//!
//! Minimising leaves the pane focused — the user just pressed a key in it — so "expand
//! as soon as we are focused" would undo the minimise on the very next update. The modal
//! must first see focus leave, and only then treat regaining it as the request to
//! expand. That is the whole of this state machine, and it is here rather than in the
//! plugin because getting it wrong is invisible until you use it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Expanded,
    Minimized { seen_unfocused: bool },
}

impl Size {
    pub fn minimized() -> Self {
        Size::Minimized {
            seen_unfocused: false,
        }
    }

    pub fn is_minimized(self) -> bool {
        matches!(self, Size::Minimized { .. })
    }

    /// Feeds in whether the modal's own pane is currently focused.
    ///
    /// Returns the next state and whether the caller should expand the pane now.
    pub fn on_focus(self, is_focused: bool) -> (Self, bool) {
        match self {
            Size::Expanded => (self, false),
            Size::Minimized { seen_unfocused } => {
                if !is_focused {
                    (
                        Size::Minimized {
                            seen_unfocused: true,
                        },
                        false,
                    )
                } else if seen_unfocused {
                    (Size::Expanded, true)
                } else {
                    (self, false)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staying_focused_right_after_minimising_does_not_expand() {
        let state = Size::minimized();
        let (state, expand) = state.on_focus(true);
        assert!(!expand, "the keypress that minimised it still holds focus");
        assert!(state.is_minimized());
    }

    #[test]
    fn regaining_focus_after_losing_it_expands() {
        let state = Size::minimized();
        let (state, expand) = state.on_focus(false);
        assert!(!expand);
        let (state, expand) = state.on_focus(true);
        assert!(expand, "focus came back, which is the request to expand");
        assert_eq!(state, Size::Expanded);
    }

    #[test]
    fn losing_focus_repeatedly_is_harmless() {
        let mut state = Size::minimized();
        for _ in 0..5 {
            let (next, expand) = state.on_focus(false);
            assert!(!expand);
            state = next;
        }
        assert!(state.is_minimized());
    }

    #[test]
    fn an_expanded_modal_never_asks_to_expand_again() {
        for focused in [true, false] {
            let (state, expand) = Size::Expanded.on_focus(focused);
            assert!(!expand);
            assert_eq!(state, Size::Expanded);
        }
    }

    #[test]
    fn expanding_settles_and_does_not_re_trigger() {
        let (state, _) = Size::minimized().on_focus(false);
        let (state, expand) = state.on_focus(true);
        assert!(expand);
        let (state, expand) = state.on_focus(true);
        assert!(!expand, "already expanded, nothing more to do");
        assert_eq!(state, Size::Expanded);
    }
}
