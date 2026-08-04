# zellij-tab-notes

One markdown note per Zellij tab. Tabs with a note are marked with an icon in the tab bar.

## Install

    rustup target add wasm32-wasip1
    ./build.sh

Then see `docs/superpowers/specs/2026-08-03-zellij-tab-notes-design.md` for the
`config.kdl` snippet.

## Usage

`Alt m` opens the note for the current tab. `e` edits it in `$EDITOR`, `d` deletes it,
`j`/`k` scroll, `Esc` closes.

## Manual test checklist

Run once after any change to the watcher or the modal.

1. Fresh session, tab with an existing non-empty note file → the icon appears at startup.
2. Tab with an empty note file → no icon.
3. `Alt m` on a tab with no note → placeholder text; `e` opens nvim on a new file.
4. Write content, `:wq` → the icon appears.
5. Open nvim on the note, delete all content, `:wq` → the icon disappears and the file
   is gone.
6. Rename a tab that has a note → the note file is renamed with it, the icon stays.
7. Move a tab (`Alt i` / `Alt o`) → no note is moved, the icon stays.
8. `d` then `y` in the modal → the note is deleted and the icon disappears.
9. Two tabs with the same name → they share one note. Renaming a tab onto a name already taken leaves both note files intact and orphans the source file rather than overwriting.
10. Delete confirmation state does not survive between tabs: arm delete with `d` on one tab, move the modal to another tab, and confirm `y` does nothing until `d` is pressed again.
