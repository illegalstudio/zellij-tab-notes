use std::collections::BTreeMap;
use std::path::PathBuf;

pub const DEFAULT_ICON: &str = "📝";

#[derive(Debug, Clone)]
pub struct Config {
    pub notes_dir: PathBuf,
    pub icon: String,
}

impl Config {
    pub fn from_map(configuration: &BTreeMap<String, String>) -> Result<Config, String> {
        let notes_dir = configuration
            .get("notes_dir")
            .ok_or_else(|| "tab-notes: missing required plugin configuration `notes_dir`".to_string())?;
        if !notes_dir.starts_with('/') {
            return Err(format!(
                "tab-notes: `notes_dir` must be an absolute path (got `{notes_dir}`). \
                 Commands run without a shell, so `~` is not expanded."
            ));
        }
        Ok(Config {
            notes_dir: PathBuf::from(notes_dir),
            icon: configuration
                .get("icon")
                .cloned()
                .unwrap_or_else(|| DEFAULT_ICON.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn reads_notes_dir_and_defaults_the_icon() {
        let config = Config::from_map(&map(&[("notes_dir", "/notes")])).unwrap();
        assert_eq!(config.notes_dir, PathBuf::from("/notes"));
        assert_eq!(config.icon, "📝");
    }

    #[test]
    fn honours_a_custom_icon() {
        let config = Config::from_map(&map(&[("notes_dir", "/notes"), ("icon", "*")])).unwrap();
        assert_eq!(config.icon, "*");
    }

    #[test]
    fn rejects_a_missing_notes_dir() {
        assert!(Config::from_map(&map(&[])).is_err());
    }

    #[test]
    fn rejects_a_relative_notes_dir_because_there_is_no_shell_to_expand_it() {
        assert!(Config::from_map(&map(&[("notes_dir", "~/notes")])).is_err());
        assert!(Config::from_map(&map(&[("notes_dir", "notes")])).is_err());
    }
}
