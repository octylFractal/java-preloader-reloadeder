#![deny(warnings)]

use std::process::exit;

use anyhow::{anyhow, Context, Result};
use colored::*;
use either::Either;
use log::{debug, info};
use structopt::StructOpt;

use crate::api::adoptium::AdoptiumApi;
use crate::api::def::JdkFetchApi;
use crate::config::Configuration;
use crate::jdk_manager::JdkManager;

mod api;
mod config;
mod content_disposition_parser;
mod jdk_manager;
mod progress;
mod util;

#[derive(StructOpt)]
#[structopt(name = "jpre", about = "A JDK management tool")]
struct Jpre {
    #[structopt(short, long, parse(from_occurrences))]
    verbose: usize,
    #[structopt(long, default_value = "https://api.adoptium.net/v3")]
    api_base_url: String,
    #[structopt(long, default_value = "adoptium")]
    api_vendor: String,
    #[structopt(subcommand)]
    cmd: Subcommand,
}

#[derive(StructOpt)]
enum Subcommand {
    #[structopt(about = "Download and use a specific JDK")]
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
    #[structopt(about = "Remove a downloaded JDK")]
    Remove {
        #[structopt(help = "The JDK to remove (major version only)")]
        jdk: String,
        #[structopt(
            short,
            long,
            help = "Force delete the JDK, do not ask for confirmation"
        )]
        force: bool,
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
    #[structopt(about = "Print the path to the JAVA_HOME symlink for the current TTY")]
    JavaHome {},
}

fn parse_jdk_or_keyword(s: &str) -> Either<u8, &str> {
    s.parse::<u8>()
        .map(Either::Left)
        .unwrap_or_else(|_| Either::Right(s))
}

fn load_default(config: &Configuration, jdk: String) -> Result<u8> {
    let jdk_or_keyword = parse_jdk_or_keyword(jdk.as_str());
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

fn load_jdk_list(
    jdk_manager: &JdkManager<AdoptiumApi>,
    config: &Configuration,
    jdk: String,
) -> Result<Vec<u8>> {
    let jdk_or_keyword = parse_jdk_or_keyword(jdk.as_str());
    match jdk_or_keyword {
        Either::Left(jdk_major) => Ok(vec![jdk_major]),
        Either::Right(unknown) => {
            if unknown == "default" {
                config.resolve_default().map(|v| vec![v])
            } else if unknown == "all" {
                jdk_manager
                    .get_all_jdk_majors()
                    .context("Failed to get all JDK major versions")
            } else {
                Err(anyhow!(
                    "Not a JDK major version, 'all', or 'default': {}",
                    unknown
                ))
            }
        }
    }
}

fn check_env_bound(jdk_manager: &JdkManager<AdoptiumApi>) -> Result<()> {
    let symlink_path = jdk_manager.get_symlink_location()?;
    let symlink = symlink_path
        .to_str()
        .context("Failed to get symlink as string")?;
    let java_home = std::env::var("JAVA_HOME").unwrap_or_else(|_| "".to_string());
    if symlink != java_home {
        eprintln!(
            "{}",
            format!(
                "Warning: JAVA_HOME is set to '{}', not the jpre symlink '{}'.\n\
                 Don't forget to export JAVA_HOME=\"$(jpre java-home)\"!",
                java_home, symlink
            )
            .yellow()
        )
    }
    Ok(())
}

fn main() {
    let args: Jpre = Jpre::from_args();
    if let Err(error) = main_for_result(args) {
        eprintln!("{}", format!("Error: {:?}", error).red());
        exit(1);
    }
}

fn main_for_result(args: Jpre) -> Result<()> {
    let mut config = Configuration::new().context("Failed to load config")?;
    stderrlog::new()
        .verbosity(args.verbose + 1)
        .init()
        .context("Failed to initialize logging")?;
    let api = AdoptiumApi {
        base_url: args.api_base_url,
        vendor: args.api_vendor,
    };
    let jdk_manager = JdkManager { api: api.clone() };
    match args.cmd {
        Subcommand::Use { jdk } => {
            check_env_bound(&jdk_manager).context("Failed to check environment variables")?;
            let jdk_major =
                load_default(&config, jdk).context("Failed to load default JDK binding")?;
            jdk_manager
                .symlink_jdk_path(jdk_major)
                .context("Failed to overwrite symlink with JDK binding")?;
            let jdk_version = jdk_manager
                .get_jdk_version(jdk_major)
                .context("Failed to get JDK version")?;
            eprintln!("{}", format!("Now using JDK {}", jdk_version).green());
        }
        Subcommand::Update { check, jdk } => {
            let majors =
                load_jdk_list(&jdk_manager, &config, jdk).context("Failed to load JDK list")?;
            let versions = jdk_manager.map_available_jdk_versions(&majors);
            let mut update_versions = Vec::new();

            for major in majors {
                if let Some((_, version)) = versions.iter().find(|(x, _)| *x == major) {
                    info!("Checking latest version of {}", version);
                    let latest_opt = api
                        .get_latest_jdk_version(major)
                        .context("Failed to get latest JDK version")?;
                    let latest = match latest_opt {
                        Some(s) => s,
                        None => continue,
                    };
                    debug!("Latest version of {} is {}", version, latest);
                    if latest != *version {
                        println!(
                            "{} {}",
                            "Update available:".green(),
                            format!(
                                "{} -> {}",
                                version.to_string().yellow(),
                                latest.to_string().cyan()
                            )
                        );
                        update_versions.push(major);
                    }
                } else {
                    println!("{}", format!("{} is not installed", major).yellow());
                    continue;
                }
            }

            if update_versions.is_empty() {
                println!("{}", "No updates available.".yellow());
            }

            if !check {
                for major in update_versions {
                    info!("Updating to latest version of {}", major);
                    jdk_manager
                        .update_jdk(major)
                        .context("Failed to update JDK")?;
                }
            }
        }
        Subcommand::Remove { jdk, force } => {
            check_env_bound(&jdk_manager).context("Failed to check environment variables")?;
            let jdk_major = match parse_jdk_or_keyword(jdk.as_str()) {
                Either::Left(jdk_major) => jdk_major,
                Either::Right(unknown) => {
                    return Err(anyhow!("Not a JDK major version: {}", unknown));
                }
            };
            let jdk_version = jdk_manager.get_jdk_version(jdk_major).unwrap_or(jdk);
            let deleted = jdk_manager
                .delete_jdk_path(jdk_major, force)
                .context("Failed to remove JDK directory")?;
            if deleted {
                eprintln!(
                    "{}",
                    format!("Successfully removed JDK {}", jdk_version).green()
                );
            }
        }
        Subcommand::List {} => {
            let majors = jdk_manager
                .get_all_jdk_majors()
                .context("Failed to load all installed JDKs")?;
            if majors.is_empty() {
                eprintln!("{}", "No JDKs installed.".yellow());
                return Ok(());
            }
            let versions = jdk_manager.map_available_jdk_versions(&majors);
            for (major, version) in versions {
                println!(
                    "{}: {}",
                    major.to_string().cyan(),
                    version.to_string().green()
                );
            }
        }
        Subcommand::Current {} => {
            check_env_bound(&jdk_manager).context("Failed to check environment variables")?;
            let jdk_version = jdk_manager
                .get_current_jdk()
                .unwrap_or_else(|_| "".to_string());
            println!("{}", jdk_version.green());
        }
        Subcommand::Default { jdk } => {
            if let Some(jdk_major) = jdk {
                jdk_manager
                    .get_jdk_path(jdk_major)
                    .context("Failed to get JDK path")?;
                config.set_default(jdk_major);
                config.save().context("Failed to save config")?;
                println!(
                    "{}",
                    format!("Updated default JDK to {}", jdk_major).green()
                );
            } else {
                match config.resolve_default() {
                    Ok(jdk_major) => {
                        println!("{}", jdk_major.to_string().green());
                    }
                    Err(err) => {
                        eprintln!("{}", err.to_string().red());
                    }
                }
            }
        }
        Subcommand::JavaHome {} => {
            let symlink_location = jdk_manager
                .get_symlink_location()
                .context("Failed to get symlink binding")?;
            if !symlink_location.exists() {
                // Initialize with default
                if let Ok(default) = config.resolve_default() {
                    jdk_manager.symlink_jdk_path(default)?;
                }
            }
            println!(
                "{}",
                symlink_location
                    .to_str()
                    .ok_or_else(|| anyhow!("Invalid symlink location"))?
                    .green()
            );
        }
    };
    Ok(())
}
