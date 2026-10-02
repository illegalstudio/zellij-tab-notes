/// A fixed script: directory names are positional arguments, never shell source.
/// Refuse a destination containing notes before moving anything. Keep the source
/// directory so an editor still using its old path can save.
pub const MIGRATE_NOTES: &str = r#"
set -eu
[ "$1" != "$2" ] || exit 0
[ ! -L "$1" ] && [ ! -L "$2" ] || exit 1
mkdir -p "$2"
# Any destination note could be adopted by a tab with no source note.
# Refuse the entire occupied namespace, including empty files and symlinks.
for target in "$2"/*.md; do
    if [ -e "$target" ] || [ -L "$target" ]; then
        printf '%s\n' 'Destination contains notes; choose a different session name.' >&2
        exit 1
    fi
done
[ -d "$1" ] || exit 0
source=$1
destination=$2
set --
rollback() {
    result=$?
    trap - EXIT HUP INT TERM
    [ "$result" -ne 0 ] || return 0
    for moved do
        original="$source/${moved##*/}"
        # An editor may have recreated the old path. Never overwrite it.
        if [ -e "$original" ] || [ -L "$original" ]; then
            printf 'Rollback blocked; recover note from %s\n' "$moved" >&2
        elif ! mv -n "$moved" "$original" || [ -e "$moved" ] || [ -L "$moved" ]; then
            printf 'Rollback failed; recover note from %s\n' "$moved" >&2
        fi
    done
    exit "$result"
}
trap 'rollback "$@"' EXIT
trap 'exit 1' HUP INT TERM
for note in "$source"/*.md; do
    [ -f "$note" ] && [ ! -L "$note" ] || continue
    target="$destination/${note##*/}"
    [ ! -e "$target" ] && [ ! -L "$target" ] || exit 1
    if mv -n "$note" "$target"; then
        set -- "$@" "$target"
    else
        exit 1
    fi
    [ ! -e "$note" ] || exit 1
done
"#;

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct Notes(PathBuf);
    impl Notes {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "tab-notes-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn migrate(&self, from: &str, to: &str) -> bool {
            Command::new("sh")
                .current_dir(&self.0)
                .args(["-c", MIGRATE_NOTES, "tab-notes-migrate"])
                .arg(self.0.join(from))
                .arg(self.0.join(to))
                .status()
                .unwrap()
                .success()
        }
    }
    impl Drop for Notes {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn partial_failure_rolls_back_and_can_retry() {
        use std::os::unix::fs::PermissionsExt;
        let n = Notes::new();
        fs::create_dir(n.0.join("old")).unwrap();
        fs::create_dir(n.0.join("bin")).unwrap();
        for name in ["a.md", "b.md"] {
            fs::write(n.0.join("old").join(name), name).unwrap();
        }
        // Fail the second forward move, but let rollback use the real mv.
        let real_mv = Command::new("sh")
            .args(["-c", "command -v mv"])
            .output()
            .unwrap();
        let real_mv = String::from_utf8(real_mv.stdout).unwrap();
        fs::write(
            n.0.join("bin/mv"),
            format!(
                "#!/bin/sh\ncase \"$2\" in */old/b.md) exit 1;; esac\nexec {} \"$@\"\n",
                real_mv.trim()
            ),
        )
        .unwrap();
        fs::set_permissions(n.0.join("bin/mv"), fs::Permissions::from_mode(0o755)).unwrap();
        let status = Command::new("sh")
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    n.0.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .args(["-c", MIGRATE_NOTES, "migration-test"])
            .arg(n.0.join("old"))
            .arg(n.0.join("new"))
            .status()
            .unwrap();
        assert!(!status.success());
        for name in ["a.md", "b.md"] {
            assert_eq!(
                fs::read_to_string(n.0.join("old").join(name)).unwrap(),
                name
            );
            assert!(!n.0.join("new").join(name).exists());
        }
        assert!(n.migrate("old", "new"));
        assert_eq!(fs::read_dir(n.0.join("new")).unwrap().count(), 2);
    }

    #[test]
    fn rollback_preserves_an_editor_save_and_reports_recovery_path() {
        use std::os::unix::fs::PermissionsExt;
        let n = Notes::new();
        fs::create_dir(n.0.join("old")).unwrap();
        fs::create_dir(n.0.join("bin")).unwrap();
        for name in ["a.md", "b.md"] {
            fs::write(n.0.join("old").join(name), "original").unwrap();
        }
        let real_mv = Command::new("sh")
            .args(["-c", "command -v mv"])
            .output()
            .unwrap();
        let real_mv = String::from_utf8(real_mv.stdout).unwrap();
        fs::write(n.0.join("bin/mv"), format!(
            "#!/bin/sh\ncase \"$2\" in */old/b.md) printf editor > \"${{2%/*}}/a.md\"; exit 1;; esac\nexec {} \"$@\"\n", real_mv.trim()
        )).unwrap();
        fs::set_permissions(n.0.join("bin/mv"), fs::Permissions::from_mode(0o755)).unwrap();
        let result = Command::new("sh")
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    n.0.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .args(["-c", MIGRATE_NOTES, "migration-test"])
            .arg(n.0.join("old"))
            .arg(n.0.join("new"))
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert_eq!(fs::read_to_string(n.0.join("old/a.md")).unwrap(), "editor");
        assert_eq!(
            fs::read_to_string(n.0.join("new/a.md")).unwrap(),
            "original"
        );
        assert_eq!(
            fs::read_to_string(n.0.join("old/b.md")).unwrap(),
            "original"
        );
        assert!(String::from_utf8(result.stderr).unwrap().contains(&format!(
            "recover note from {}",
            n.0.join("new/a.md").display()
        )));
    }

    #[test]
    fn listing_failure_is_retryable_and_missing_directory_is_empty() {
        let n = Notes::new();
        let list = || {
            Command::new("sh")
                .args(["-c", crate::listing::LIST_NOTES, "listing-test"])
                .arg(n.0.join("notes"))
                .output()
                .unwrap()
        };
        let missing = list();
        assert!(missing.status.success());
        assert!(missing.stdout.is_empty());
        fs::write(n.0.join("notes"), "not a directory").unwrap();
        assert!(!list().status.success());
        fs::remove_file(n.0.join("notes")).unwrap();
        fs::create_dir(n.0.join("notes")).unwrap();
        fs::write(n.0.join("notes/test.md"), "note").unwrap();
        let retry = list();
        assert!(retry.status.success());
        assert!(String::from_utf8(retry.stdout).unwrap().contains("test.md"));
    }

    #[test]
    fn follows_session_renames_and_keeps_old_directory_for_open_editors() {
        let n = Notes::new();
        fs::create_dir(n.0.join("scratch")).unwrap();
        fs::write(
            n.0.join("scratch/review.md"),
            "https://github.com/example/repo/pull/1",
        )
        .unwrap();
        assert!(n.migrate("scratch", "task"));
        assert!(n.migrate("task", "renamed again"));
        assert_eq!(
            fs::read_to_string(n.0.join("renamed again/review.md")).unwrap(),
            "https://github.com/example/repo/pull/1"
        );
        assert!(n.0.join("scratch").is_dir());
        assert!(!n.0.join("scratch/review.md").exists());
    }

    #[test]
    fn refuses_occupied_destination_before_moving_any_note() {
        let n = Notes::new();
        for dir in ["old", "new"] {
            fs::create_dir(n.0.join(dir)).unwrap();
        }
        fs::write(n.0.join("old/shared.md"), "source").unwrap();
        fs::write(n.0.join("new/shared.md"), "destination").unwrap();
        fs::write(n.0.join("old/unique.md"), "keep").unwrap();
        fs::write(n.0.join("old/empty.md"), "").unwrap();
        assert!(!n.migrate("old", "new"));
        assert_eq!(
            fs::read_to_string(n.0.join("old/shared.md")).unwrap(),
            "source"
        );
        assert_eq!(
            fs::read_to_string(n.0.join("new/shared.md")).unwrap(),
            "destination"
        );
        assert!(n.0.join("old/unique.md").is_file());
        assert!(n.0.join("old/empty.md").is_file());
    }

    #[test]
    fn names_are_data_not_shell_code_and_same_directory_is_a_noop() {
        let n = Notes::new();
        let from = "scratch ' $(touch PWNED); `id`";
        let to = "task \" $HOME";
        fs::create_dir(n.0.join(from)).unwrap();
        fs::write(n.0.join(from).join("$(touch PWNED).md"), "note").unwrap();
        assert!(n.migrate(from, to));
        assert!(n.migrate(to, to));
        assert_eq!(
            fs::read_to_string(n.0.join(to).join("$(touch PWNED).md")).unwrap(),
            "note"
        );
        assert!(!n.0.join("PWNED").exists());
    }

    #[test]
    fn missing_source_is_normal_and_symlink_directory_is_rejected() {
        let n = Notes::new();
        assert!(n.migrate("missing", "new"));
        std::os::unix::fs::symlink(n.0.join("new"), n.0.join("link")).unwrap();
        assert!(!n.migrate("link", "other"));
        assert!(!n.migrate("missing", "link"));
    }

    #[test]
    fn does_not_follow_note_symlinks() {
        let n = Notes::new();
        for dir in ["old", "new"] {
            fs::create_dir(n.0.join(dir)).unwrap();
        }
        fs::write(n.0.join("outside"), "untouched").unwrap();
        std::os::unix::fs::symlink(n.0.join("outside"), n.0.join("old/link.md")).unwrap();
        fs::write(n.0.join("old/shared.md"), "keep").unwrap();
        std::os::unix::fs::symlink(n.0.join("absent"), n.0.join("new/shared.md")).unwrap();
        assert!(!n.migrate("old", "new"));
        assert!(n.0.join("old/shared.md").is_file());
        assert!(n.0.join("old/link.md").is_symlink());
        assert_eq!(
            fs::read_to_string(n.0.join("outside")).unwrap(),
            "untouched"
        );
    }
}
