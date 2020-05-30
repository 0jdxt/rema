use crate::errors::ConfigError;

use std::convert::TryFrom;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use git2::Repository;
use serde::Deserialize;

// Config for building a repo
#[derive(Deserialize)]
pub(crate) struct RemaConfig {
    #[serde(skip)]
    repo: Option<Repository>,
    #[serde(default)]
    build: Vec<String>,
    #[serde(default)]
    clean: Vec<String>,
    #[serde(default)]
    autoclean: bool,
    #[serde(default)]
    autoupdate: bool,
}

impl fmt::Debug for RemaConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repo = self.repo.as_ref().map(|r| r.path().to_str());
        write!(
            f,
            "{:?} b:{:?} c:{:?} up:{} cl:{}",
            repo, self.build, self.clean, self.autoupdate, self.autoclean
        )
    }
}

// Create RemaConfig from path to repository
impl TryFrom<PathBuf> for RemaConfig {
    type Error = ConfigError;

    fn try_from(p: PathBuf) -> Result<Self, Self::Error> {
        let f = p.join("rema.toml");
        let mut c: Self = toml::from_str(&fs::read_to_string(f).unwrap()).unwrap();
        c.repo = Some(Repository::open(p).unwrap());
        Ok(c)
    }
}

impl RemaConfig {
    pub(crate) fn path(&self) -> &Path {
        self.repo.as_ref().unwrap().path()
    }

    // returns wether update needed or not
    pub(crate) fn pull(&self) -> bool {
        let output = std::process::Command::new("git")
            .current_dir(self.path())
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
        println!("exec: {} {:?} in {:?}", cmd, args, self.path());

        std::process::Command::new(cmd)
            .current_dir(self.path())
            .args(args)
            .spawn()
            .expect("failed to run command")
            .wait()
            .expect("command failed to run");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq for RemaConfig {
        fn eq(&self, other: &Self) -> bool {
            self.build == other.build
                && self.clean == other.clean
                && self.autoupdate == other.autoupdate
                && self.clean == other.clean
        }
    }

    #[test]
    fn test_rema_config_full() {
        let config = r#"
                build = ["cmd1", "cmd2"]
                clean = ["clean pls"]
                autoclean = true
                autoupdate = true
            "#;

        // check config is parsed correctly
        let conf: RemaConfig = toml::from_str(&config).unwrap();
        let expected = RemaConfig {
            repo: None,
            build: vec!["cmd1".into(), "cmd2".into()],
            clean: vec!["clean pls".into()],
            autoupdate: true,
            autoclean: true,
        };
        assert_eq!(conf, expected);
    }

    #[test]
    fn test_invalid_config_no_panic() {
        let config = r#"
            [dir.home]
            path = "~"
            "#;

        let conf: RemaConfig = toml::from_str(&config).unwrap();
        let expected = RemaConfig {
            repo: None,
            build: vec![],
            clean: vec![],
            autoclean: false,
            autoupdate: false,
        };
        assert_eq!(conf, expected);
    }
}
