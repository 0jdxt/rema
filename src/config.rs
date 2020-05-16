use crate::errors::ConfigError;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use git2::Repository;
use serde::{Deserialize, Deserializer};

// TODO:
// let conf be in .config/rema/config.toml, add option for config file

// info for each dir to look through
#[derive(Debug, Deserialize, PartialEq, Eq)]
struct RemaDir {
    #[serde(deserialize_with = "expand")]
    path: PathBuf,
    #[serde(default)]
    include: HashSet<String>,
    #[serde(default)]
    ignore: HashSet<String>,
    #[serde(default)]
    autoclean: bool,
    #[serde(default)]
    autoupdate: bool,
}

// Rema config
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct RemaConfig {
    #[serde(rename = "dir")]
    dirs: HashMap<String, RemaDir>,
    #[serde(default)]
    autoclean: bool,
    #[serde(default)]
    autoupdate: bool,
}

impl RemaConfig {
    /// Creates a new RemaConfig instance, validating data
    pub(crate) fn new(config_match: Option<&str>) -> Result<Self, ConfigError> {
        // if custom config not specified, fallback to CONFIG_DIR/config.toml
        // if dirs::config_dir fails, path to config needs to be provided
        let path: PathBuf = if let Some(s) = config_match {
            s.into()
        } else {
            let mut config_dir = dirs::config_dir().expect("could not find user config directory.");
            config_dir.push("rema/config.toml");
            config_dir
        };

        // bail on io error or deserialise error
        let buf = std::fs::read_to_string(path).map_err(ConfigError::from)?;
        let config: Self = toml::from_str(&buf).map_err(ConfigError::from)?;

        // check dirs are suitable
        for RemaDir { path, .. } in config.dirs.values() {
            if path.is_relative() {
                return Err(ConfigError::BaseDirRelative(path.clone()));
            } else if !path.is_dir() {
                return Err(ConfigError::BaseDirNotDir(path.clone()));
            }
        }
        Ok(config)
    }
}

// Iterator over directories in the base directory
impl IntoIterator for RemaConfig {
    type Item = Repository;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    // TODO: redo to use correct ignore
    fn into_iter(self) -> Self::IntoIter {
        self.dirs
            .values()
            .map(|RemaDir { path, .. }| {
                std::fs::read_dir(path).expect("base_dir should be directory")
            })
            .flatten()
            .filter_map(|entry| match entry {
                Ok(e) => {
                    // check entry is directory and not in ignores list
                    let _s = e.file_name().into_string().unwrap();
                    if e.path().is_file() {
                        None
                    } else {
                        Repository::open(e.path()).ok()
                    }
                }
                Err(_) => None,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

// Config for building a repo
#[derive(Debug, Deserialize)]
pub(crate) struct BuildConfig {
    name: String,
    #[serde(rename = "name")]
    path: PathBuf,
    #[serde(default)]
    build: Vec<String>,
    #[serde(default)]
    clean: Vec<String>,
    #[serde(default)]
    autoclean: bool,
    #[serde(default)]
    autoupdate: bool,
}

impl<'c> BuildConfig {
    pub(crate) fn new(p: PathBuf) -> Self {
        Self {
            name: p.to_str().unwrap().to_string(),
            path: p,
            build: vec![],
            clean: vec![],
            autoclean: false,
            autoupdate: false,
        }
    }

    pub(crate) fn to_path(&self) -> PathBuf {
        self.path.to_path_buf()
    }

    pub(crate) fn try_from_repo(repo: &'c Repository) -> Option<Self> {
        let p = repo.path().to_path_buf();
        let f = p.join("rema.toml");

        if let Ok(buf) = std::fs::read_to_string(f) {
            toml::from_str::<BuildConfig>(&buf).ok()
        } else {
            Some(Self::new(p))
        }
    }

    // returns wether update needed or not
    pub(crate) fn pull(&self) -> bool {
        let output = std::process::Command::new("git")
            .current_dir(self.path.as_path())
            .arg("pull")
            .output()
            .expect("failed to execute git");

        let check_phrase = "Already up to date.";
        let check = String::from_utf8(output.stdout[..check_phrase.len()].to_vec()).unwrap();

        if self.autoupdate {
            self.build();
            false
        } else {
            output.status.success() && check != check_phrase
        }
    }

    pub(crate) fn build(&self) {
        for line in &self.build {
            self.run_line_as_cmd(line);
        }

        if self.autoclean {
            self.clean()
        }
    }

    pub(crate) fn clean(&self) {
        for line in &self.clean {
            self.run_line_as_cmd(line);
        }
    }

    fn run_line_as_cmd(&self, line: &str) {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        let (cmd, args) = parts.as_slice().split_first().unwrap();
        println!("exec: {} {:?} in {:?}", cmd, args, self.path);

        std::process::Command::new(cmd)
            .current_dir(self.path.as_path())
            .args(args)
            .spawn()
            .expect("failed to run command")
            .wait()
            .expect("command failed to run");
    }
}

fn expand<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(s.expand().into())
}

// Helper trait to shellexpand paths from configs
trait ShellExpand {
    fn expand(&self) -> Self;
}

impl ShellExpand for String {
    fn expand(&self) -> Self {
        match shellexpand::full(self) {
            Ok(p) => p,
            Err(e) => panic!("Couldn't expand path, got error: {:?}", e),
        }
        .into()
    }
}

impl<T: ShellExpand> ShellExpand for Vec<T> {
    fn expand(&self) -> Self {
        self.iter().map(ShellExpand::expand).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::{hashmap, hashset};

    const HOME: &str = "/home/randomuser";

    fn set_home() {
        // set $HOME variable for testing
        std::env::set_var("HOME", HOME);
    }

    #[test]
    fn test_rema_config_full() {
        set_home();

        let config = r#"
                autoclean = true
                autoupdate = true

                [dir.home]
                path = "~"
                ignore = ["a", "b", "c"]
                autoclean = true
                autoupdate = true
            "#;

        // check config is parsed correctly
        let conf: RemaConfig = toml::from_str(&config).unwrap();
        let expected = RemaConfig {
            dirs: hashmap! {
                "home".into() => RemaDir {
                    path: HOME.into(),
                    include: hashset!{},
                    ignore: hashset! {"a".into(), "b".into(), "c".into()},
                    autoclean: true,
                    autoupdate: true,
                }
            },
            autoclean: true,
            autoupdate: true,
        };
        assert_eq!(conf, expected);
    }

    #[test]
    fn test_rema_config_minimal() {
        set_home();

        let config = r#"
            [dir.home]
            path = "~"
            "#;

        let conf: RemaConfig = toml::from_str(&config).unwrap();
        let expected = RemaConfig {
            dirs: hashmap! {
                "home".into() => RemaDir {
                    path: HOME.into(),
                    include: hashset!{},
                    ignore: hashset!{},
                    autoclean: false,
                    autoupdate: false,
                }
            },
            autoclean: false,
            autoupdate: false,
        };
        assert_eq!(conf, expected);
    }

    #[test]
    #[should_panic]
    fn test_invalid_config() {
        RemaConfig::new(Some("invalid.toml")).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_config_path() {
        RemaConfig::new(Some("nonexistantfile")).unwrap();
    }
}
