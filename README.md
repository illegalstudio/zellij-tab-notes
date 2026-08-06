# zellij-tab-notes

[![CI](https://github.com/illegalstudio/zellij-tab-notes/actions/workflows/ci.yml/badge.svg)](https://github.com/illegalstudio/zellij-tab-notes/actions/workflows/ci.yml)

One markdown note per Zellij tab. Tabs with a note are marked with an icon in the tab bar.

## Install

### From a release — no Rust toolchain needed

Zellij loads plugins straight from an `https://` location and follows redirects, so a
release asset is a working plugin location. Point `config.kdl` at it and there is
nothing to build:

```kdl
plugins {
    tab-notes location="https://github.com/illegalstudio/zellij-tab-notes/releases/download/v0.1.0/tab-notes.wasm" {
        role "modal"
        notes_dir "/Users/you/.local/share/zellij-tab-notes"
        icon "📝"
    }
    tab-notes-watcher location="https://github.com/illegalstudio/zellij-tab-notes/releases/download/v0.1.0/tab-notes.wasm" {
        role "watcher"
        notes_dir "/Users/you/.local/share/zellij-tab-notes"
        icon "📝"
    }
}
```

Replace `v0.1.0` with a release that actually exists — check the releases page. And pin
a version rather than using `releases/latest/download/`: Zellij caches plugins by URL,
so a floating URL changes what you are running without you asking. Each release ships a
`tab-notes.wasm.sha256` next to the binary if you want to check it.

> **If you are working on the plugin, do not install it this way.** A release URL is
> immutable by design, so Zellij keeps loading the published build and your local
> rebuilds have no effect whatsoever — with no error to tell you so. Use the `file:`
> location below while developing.

### From source

    ./build.sh

The pinned toolchain and the `wasm32-wasip1` target come from `rust-toolchain.toml`;
rustup installs them on first build. `build.sh` writes the wasm to
`~/.local/share/zellij/plugins/tab-notes.wasm`, which the `file:` location below points
at.

`notes_dir` must be an absolute path in either case: commands run without a shell, so a
leading `~` would not be expanded and you would get a directory literally named `~`.

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
```

### Then, whichever method you used

```kdl
// The watcher keeps the icons correct from the moment the session opens, so it has to
// be running before the modal is ever opened.
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
`$EDITOR`, `d` deletes it, `m` minimises it, `f` hands focus back to the terminal,
`j`/`k` scroll, `Esc` closes.

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
12. `m` in the modal → it shrinks to a pinned box in the top-right corner, stays
    readable, and stays on top while you move around other panes. `Ctrl t` `a` focuses
    it again and `m` restores the full size — the exact size and position it had, which
    the modal records from pane updates while it is expanded. While minimised the key
    hints appear only when the box has focus, since that is the only time those keys
    reach it.
13. `f` in the modal → focus returns to the pane you were working in and the box stays
    put, minimised and pinned if that is how you left it.
14. Rename a tab to `feature/login` → the note file is `feature-login.md` and the tab shows
    the icon with its slash intact.

## Releasing

CI runs formatting, clippy, the core tests and the wasm build on every push and pull
request.

Releases are **tag-driven**: the tag is the version, and the manifest follows it. There
is nothing to bump by hand.

```sh
git tag v0.2.0
git push origin v0.2.0
```

The release workflow then derives `0.2.0` from the tag, writes it into
`[workspace.package]` in the root `Cargo.toml` — both crates inherit from there — runs
the tests, builds the wasm, publishes the release with `tab-notes.wasm` and its
`.sha256`, and commits the bump back to `main`.

Two consequences worth knowing:

- **The tagged commit still carries the previous version.** The bump lands on `main`
  after the tag, so the commit the tag points at is not the one that names the release.
  The published artifact is built from the tagged tree with the version patched in.
- **That bump commit is made by GitHub Actions**, which has no signing key, so it is
  the one unsigned commit in an otherwise signed history.

A tag whose commit is not an ancestor of `main` still produces a release, but `main` is
left untouched rather than having its version rewritten from a side branch.

Malformed tags are rejected before anything is built: `v1.2` and `vX.Y.Z` fail,
`v0.2.0` and `v0.2.0-rc1` pass.

CI proves the code compiles and that the core logic passes its tests. It cannot prove
the plugin behaves correctly inside Zellij — no automated test reaches that boundary,
and that is exactly where this project's two worst bugs lived. Run the checklist above
before tagging.

## License

MIT — see [LICENSE](LICENSE).
