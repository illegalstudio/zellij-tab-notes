# zellij-tab-notes

One markdown note per Zellij tab. Tabs with a note are marked with an icon in the tab bar.

## Install

    rustup target add wasm32-wasip1
    ./build.sh

`build.sh` installs the wasm to `~/.local/share/zellij/plugins/tab-notes.wasm`.

Then add this to `config.kdl`, adjusting the paths — `notes_dir` must be absolute,
because commands run without a shell and a leading `~` would not be expanded:

```kdl
plugins {
    tab-notes location="file:/Users/you/.local/share/zellij/plugins/tab-notes.wasm" {
        role "modal"
        notes_dir "/Users/you/.local/share/zellij-tab-notes"
        icon "📝"
    }
    tab-notes-watcher location="file:/Users/you/.local/share/zellij/plugins/tab-notes.wasm" {
        role "watcher"
        notes_dir "/Users/you/.local/share/zellij-tab-notes"
        icon "📝"
    }
}

// The watcher keeps the icons correct from the moment the session opens, so it has
// to be running before the modal is ever opened.
load_plugins {
    tab-notes-watcher
}
```

and, inside the `tab` keybinds block:

```kdl
bind "a" { LaunchOrFocusPlugin "tab-notes" { floating true; move_to_focused_tab true; }; SwitchToMode "normal"; }
```

## Usage

`Ctrl t` then `a` (annotate) opens the note for the current tab. `e` edits it in
`$EDITOR`, `d` deletes it, `j`/`k` scroll, `Esc` closes.

The binding must end with `SwitchToMode "normal"`. Zellij routes keys to the active
mode's bindings, so a client left in tab mode never delivers `e`, `d` or `j` to the
modal.

## Manual test checklist

Run once after any change to the watcher or the modal.

1. Fresh session, tab with an existing non-empty note file → the icon appears at startup.
2. Tab with an empty note file → no icon.
3. `Ctrl t` `a` on a tab with no note → placeholder text; `e` opens nvim on a new file.
4. Write content, `:wq` → nvim closes, the modal is showing again with the text you
   just wrote, and the icon appears.
5. Open nvim on the note, delete all content, `:wq` → the icon disappears and the file
   is gone.
6. Rename a tab that has a note → the note file is renamed with it, the icon stays.
7. Move a tab (`Alt i` / `Alt o`) → no note is moved, the icon stays.
8. `d` then `y` in the modal → the note is deleted and the icon disappears.
9. Two tabs with the same name → they share one note. Renaming a tab onto a name already taken leaves both note files intact and orphans the source file rather than overwriting.
10. Delete confirmation state does not survive between tabs: arm delete with `d` on one tab, move the modal to another tab, and confirm `y` does nothing until `d` is pressed again.
11. With three or more tabs, give a note to the *last* one → only that tab gains the icon,
    and the tab bar settles immediately (no flicker, no repeated renaming). This is what
    a position-based rename would break.
12. Rename a tab to `feature/login` → the note file is `feature-login.md` and the tab shows
    the icon with its slash intact.
