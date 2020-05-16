use std::error::Error;
use std::fmt;
use std::path::PathBuf;

// https://github.com/BurntSushi/imdb-rename/blob/master/src/main.rs
// Return a prettily formatted error, including its entire causal chain.
pub fn pretty_error(err: &failure::Error) -> String {
    let mut pretty = err.to_string();
    let mut prev = err.as_fail();
    while let Some(next) = prev.cause() {
        pretty.push_str(": ");
        pretty.push_str(&next.to_string());
        prev = next;
    }
    pretty
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

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::File(e.into())
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::Toml(e.into())
    }
}
