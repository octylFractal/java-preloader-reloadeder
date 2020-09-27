#![deny(warnings)]

use anyhow::{anyhow, Result};
use either::Either;
use structopt::StructOpt;
use crate::config::Configuration;

mod adoptjdk;
mod config;
mod content_disposition_parser;
mod jdk_manager;
mod reqwest_failure;

#[derive(StructOpt)]
#[structopt(name = "jpre", about = "A JDK management tool")]
struct Jpre {
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,
    #[structopt(subcommand)]
    cmd: Subcommand,
}

#[derive(StructOpt)]
enum Subcommand {
    #[structopt(about = "Use a specific JDK")]
    Use {
        #[structopt(help = "The JDK to use (major version only) or 'default'")]
        jdk: String,
    },
    #[structopt(about = "Update one or all JDKs")]
    Update {
        #[structopt(short, long, help = "Check only, do not download new updates")]
        check: bool,
        #[structopt(help = "The JDK to update (major version only), 'all', or 'default'")]
        jdk: String,
    },
    #[structopt(about = "List downloaded JDKs")]
    List {},
    #[structopt(about = "Print currently active JDK version (full)")]
    Current {},
    #[structopt(about = "Configure the default JDK")]
    Default {
        #[structopt(help = "The JDK to set as the default (major version only),\
                            or nothing to get the default")]
        jdk: Option<u8>,
    },
}

fn parse_jdk_or_keyword(s: String) -> Either<u8, String> {
    s.parse::<u8>().map(Either::Left).unwrap_or_else(|_| Either::Right(s))
}

fn load_default(config: &Configuration, jdk: String) -> Result<u8> {
    let jdk_or_keyword = parse_jdk_or_keyword(jdk);
    match jdk_or_keyword {
        Either::Left(jdk_major) => Ok(jdk_major),
        Either::Right(unknown) => {
            if unknown == "default" {
                config.resolve_default()
            } else {
                Err(anyhow!("Not a JDK major version or 'default': {}", unknown))
            }
        }
    }
}

fn load_jdk_list(config: &Configuration, jdk: String) -> Result<Vec<u8>> {
    let jdk_or_keyword = parse_jdk_or_keyword(jdk);
    match jdk_or_keyword {
        Either::Left(jdk_major) => Ok(vec![jdk_major]),
        Either::Right(unknown) => {
            if unknown == "default" {
                config.resolve_default().map(|v| vec![v])
            } else if unknown == "all" {
                jdk_manager::get_all_jdk_majors()
            } else {
                Err(anyhow!("Not a JDK major version, 'all', or 'default': {}", unknown))
            }
        }
    }
}

const CURRENT_ENV_VAR: &str = "JPRE_JAVA_VERSION";

fn main() -> Result<()> {
    let mut config = Configuration::new()?;
    let args = Jpre::from_args();
    stderrlog::new().verbosity(args.verbose).init()?;
    match args.cmd {
        Subcommand::Use { jdk } => {
            let jdk_major = load_default(&config, jdk)?;
            let path = jdk_manager::get_jdk_path(jdk_major)?;
            let jdk_version = jdk_manager::get_jdk_version(jdk_major)
                .ok_or_else(|| anyhow!("Unable to get current JDK version"))?;
            println!("export JAVA_HOME={}", path.canonicalize()?.display());
            println!("export {}={}", CURRENT_ENV_VAR, jdk_version);
        }
        Subcommand::Update { check, jdk } => {
            let majors = load_jdk_list(&config, jdk)?;
            let versions = jdk_manager::map_available_jdk_versions(&majors);
            let mut update_versions = Vec::new();

            for major in majors {
                if let Some((_, version)) = versions.iter().filter(|(x, _)| *x == major).next() {
                    let latest = adoptjdk::get_latest_jdk_version(major)?;
                    if latest != *version {
                        println!("Update available: {} -> {}", version, latest);
                        update_versions.push(major);
                    }
                } else  {
                    println!("{} is not installed", major);
                    continue
                }
            }

            if update_versions.is_empty() {
                println!("No updates available.");
            }

            if !check {
                for major in update_versions {
                    jdk_manager::update_jdk(major)?;
                }
            }
        }
        Subcommand::List {} => {
            let majors = jdk_manager::get_all_jdk_majors()?;
            let versions = jdk_manager::map_available_jdk_versions(&majors);
            for (major, version) in versions {
                println!("{}: {}", major, version);
            }
        }
        Subcommand::Current {} => {
            let jdk_version = std::env::var(CURRENT_ENV_VAR)
                .unwrap_or_else(|_| "".to_string());
            println!("{}", jdk_version);
        }
        Subcommand::Default { jdk } => {
            if let Some(jdk_major) = jdk {
                jdk_manager::get_jdk_path(jdk_major)?;
                config.set_default(jdk_major);
                config.save()?;
            } else {
                match config.resolve_default() {
                    Ok(jdk_major) => {
                        println!("{}", jdk_major);
                    }
                    Err(err) => {
                        eprintln!("{}", err);
                    }
                }
            }
        }
    };
    Ok(())
}
