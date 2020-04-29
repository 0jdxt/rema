#![warn(clippy::all, clippy::pedantic, rust_2018_idioms)]

pub(crate) mod config;

use std::fs;
use std::path::PathBuf;

use clap::clap_app;
use config::BuildConfig;
use git2::Repository;

fn main() {
    let matches = clap_app!(rema =>
        (version: clap::crate_version!())
        (author: clap::crate_authors!())
        (about: clap::crate_description!())
        (@arg CONFIG: -c --config +takes_value "Sets custom config file")
        (@subcommand pull => (about: "fetch repos updates"))
        (@subcommand update => (about: "build updated repos"))
        (@subcommand clean => (about: "clean updated repos"))
    )
    .get_matches();

    let rema_config = match config::RemaConfig::new(matches.value_of("CONFIG")) {
        Ok(c) => c,
        Err(e) => panic!("{}", pretty_error(&e.into())),
    };
    println!("{:?}", rema_config);
    // TODO: maybe tmp or idk
    let updates_file = PathBuf::new();

    macro_rules! get_config {
        ($dir:expr) => {{
            if let Some(v) = BuildConfig::try_from_repo(&$dir) {
                v
            } else {
                continue;
            }
        };};
    }

    match matches.subcommand() {
        ("pull", _) => {
            let pulls = rema_config
                .into_iter()
                .filter_map(|repo| {
                    BuildConfig::try_from_repo(&repo).and_then(|conf| {
                        if conf.pull() {
                            Some(conf.to_path())
                        } else {
                            None
                        }
                    })
                })
                .collect::<Vec<_>>();

            let ron = ron::ser::to_string(&pulls).unwrap();
            fs::write(updates_file, ron).expect("failed writing pulls to file");
        }
        ("update", _) => {
            let buf = fs::read_to_string(updates_file).expect("failed reading pulls");
            let repos: Vec<Repository> = ron::de::from_str::<Vec<PathBuf>>(&buf)
                .unwrap()
                .iter()
                .filter_map(|p| Repository::open(p).ok())
                .collect();

            for repo in repos {
                println!("build!");
                get_config!(repo).build();
            }
        }
        ("clean", _) => {
            for dir in rema_config {
                println!("clean!");
                get_config!(dir).clean();
            }
        }
        ("", None) => eprintln!("No command given"),
        (s, _) => {
            unreachable!("got subcommand: {}", s);
        }
    }
}

// https://github.com/BurntSushi/imdb-rename/blob/master/src/main.rs
// /// Return a prettily formatted error, including its entire causal chain.
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
