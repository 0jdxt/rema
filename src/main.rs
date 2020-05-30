#![warn(clippy::all, clippy::pedantic, rust_2018_idioms)]

pub(crate) mod config;
pub(crate) mod errors;

use crate::errors::pretty_error;

use std::fs;
use std::path::PathBuf;

use clap::clap_app;
use config::RemaConfig;

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

    // TODO: maybe tmp or idk
    let updates_file = PathBuf::new();

    match matches.subcommand() {
        ("pull", _) => todo!("pull repos"),
        ("update", _) => todo!("run build cmds on updated repos"),
        ("clean", _) => todo!("clean repos"),
        ("", None) => eprintln!("No command given"),
        (s, _) => {
            unreachable!("got subcommand: {}", s);
        }
    }
}
