use crate::pretty_error;

use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use git2::Repository;
use serde::Deserialize;

// TODO:
// let conf be in .config/rema/config.toml, add option for config file
// let base_dir choosable, default data_local_dir .local/share/rema/ (directory for repos)
// 'add' command to automatically pull from repos => suckless, github, gitlab etc.
// built in repo config editor? or just unify configs into main

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Repo {
    name: String,
}

// RemaConfig builder
#[derive(Debug, Deserialize, PartialEq, Eq)]
struct RemaConfigSe {
    base_dir: String,
    ignore: Option<HashSet<String>>,
    autoclean: Option<bool>,
    autoupdate: Option<bool>,
    #[serde(rename = "repo")]
    repos: HashMap<String, Repo>,
}

// stores config
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RemaConfig {
    base_dir: PathBuf,
    ignore: HashSet<String>,
    autoclean: bool,
    autoupdate: bool,
}

impl From<RemaConfigSe> for RemaConfig {
    fn from(rcs: RemaConfigSe) -> Self {
        Self {
            base_dir: rcs.base_dir.expand().into(),
            ignore: rcs.ignore.unwrap_or_default(),
            autoclean: rcs.autoclean.unwrap_or_default(),
            autoupdate: rcs.autoupdate.unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ConfigError {
    BaseDirRelative(PathBuf),
    BaseDirNotDir(PathBuf),
    File(failure::Error),
    Toml(failure::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseDirRelative(p) => {
                write!(f, "base_dir in {:?} cannot be relative", p.to_str())
            }
            Self::BaseDirNotDir(p) => {
                write!(f, "base_dir must be a directory, found: {:?}", p.to_str())
            }
            Self::File(e) => write!(f, "could not read config file: {}", pretty_error(e)),
            Self::Toml(e) => write!(f, "error in config file: {}", pretty_error(e)),
        }
    }
}
impl Error for ConfigError {}

impl RemaConfig {
    /// Creates a new RemaConfig instance, validating data
    pub(crate) fn new(config_match: Option<&str>) -> Result<Self, ConfigError> {
        // if custom config not specified, fallback to CONFIG_DIR/rema.yml
        let path: PathBuf = if let Some(s) = config_match {
            s.into()
        } else {
            // TODO: handle properly, maybe cmd option
            let mut config_dir = dirs::config_dir().expect("could not find user config directory.");
            config_dir.push("rema.conf");
            config_dir
        };

        let buf = std::fs::read_to_string(path).map_err(|e| ConfigError::File(e.into()))?;

        let config: Self = toml::from_str::<RemaConfigSe>(&buf)
            .map(|rcs| rcs.into())
            .map_err(|e| ConfigError::Toml(e.into()))?;

        if config.base_dir.is_relative() {
            Err(ConfigError::BaseDirRelative(config.base_dir))
        } else if !config.base_dir.is_dir() {
            Err(ConfigError::BaseDirNotDir(config.base_dir))
        } else {
            Ok(config)
        }
    }
}

// Iterator over directories in the base directory
impl IntoIterator for RemaConfig {
    type Item = Repository;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        std::fs::read_dir(self.base_dir.clone())
            .expect("base_dir should be directory")
            .filter_map(|entry| match entry {
                Ok(e) => {
                    // check entry is directory and not in ignores list
                    let s = e.file_name().into_string().unwrap();
                    if e.path().is_file() || self.ignore.contains(&s) {
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

// BuildConfig builder
// Result<T> types are optional in the rema.toml file
#[derive(Debug, Deserialize)]
pub(crate) struct BuildConfigSe {
    name: String,
    build: Vec<String>,
    clean: Option<Vec<String>>,
    autoclean: Option<bool>,
    autoupdate: Option<bool>,
}

// Config for building a repos
#[derive(Debug)]
pub(crate) struct BuildConfig<'c> {
    name: String,
    path: &'c Path,
    build: Vec<String>,
    clean: Vec<String>,
    autoclean: bool,
    autoupdate: bool,
}

impl<'c> From<(BuildConfigSe, &'c Path)> for BuildConfig<'c> {
    fn from((bcs, p): (BuildConfigSe, &'c Path)) -> Self {
        Self {
            name: bcs.name,
            path: p,
            build: bcs.build.expand(),
            clean: bcs.clean.unwrap_or_default().expand(),
            autoclean: bcs.autoclean.unwrap_or_default(),
            autoupdate: bcs.autoupdate.unwrap_or_default(),
        }
    }
}

impl<'c> BuildConfig<'c> {
    pub(crate) fn new(p: &'c Path) -> Self {
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
        let p = repo.path().to_path_buf().join("rema.toml");

        if let Ok(buf) = std::fs::read_to_string(p.clone()) {
            println!("Found config: {:?}", p.into_os_string());

            match toml::from_str::<BuildConfigSe>(&buf) {
                Ok(c) => {
                    println!("{:#?}", c);
                    Some((c, repo.path()).into())
                }
                Err(e) => {
                    eprintln!("Error reading config to yaml: {}", e.to_string());
                    None
                }
            }
        } else {
            Some(Self::new(repo.path()))
        }
    }

    // returns wether update needed or not
    pub(crate) fn pull(&self) -> bool {
        let output = std::process::Command::new("git")
            .current_dir(self.path)
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
            .current_dir(self.path)
            .args(args)
            .spawn()
            .expect("failed to run command")
            .wait()
            .expect("command failed to run");
    }
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

    // TODO: config test macro or function??

    #[test]
    fn test_rema_config_full() {
        set_home();

        let config = r#"
                base_dir = "~"
                ignore = ["a", "b", "c"]
                autoclean = true
                autoupdate = true"#;

        // check config is parsed correctly
        let conf: RemaConfigSe = toml::from_str(&config).unwrap();
        let expected = RemaConfigSe {
            base_dir: "~".into(),
            ignore: Some(hashset! {"a".into(), "b".into(), "c".into()}),
            autoclean: Some(true),
            autoupdate: Some(true),
            repos: HashMap::new(),
        };
        assert_eq!(conf, expected);

        // check `into` has converted properly
        let conf: RemaConfig = conf.into();
        let expected = RemaConfig {
            base_dir: HOME.into(),
            ignore: hashset! {"a".into(), "b".into(), "c".into()},
            autoclean: true,
            autoupdate: true,
        };
        assert_eq!(conf, expected);
    }

    #[test]
    fn test_rema_config_minimal() {
        set_home();

        let config = r#"
            base_dir = "~"
            [repo.test]
            name = "test repo"
            "#;
        let conf: RemaConfigSe = toml::from_str(config).unwrap();
        let expected = RemaConfigSe {
            base_dir: "~".into(),
            ignore: None,
            autoupdate: None,
            autoclean: None,
            repos: hashmap! {
                    "test".into() =>
                    Repo {
                        name: "test repo".into(),
                    },
            },
        };
        assert_eq!(conf, expected);

        let conf: RemaConfig = conf.into();
        let expected = RemaConfig {
            base_dir: HOME.into(),
            ignore: hashset! {},
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
}
