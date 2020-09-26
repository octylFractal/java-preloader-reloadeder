#![deny(warnings)]
#[macro_use]
extern crate lazy_static;

use std::error::Error;

use either::Either;
use structopt::StructOpt;

mod adoptjdk;
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
        #[structopt(help = "The JDK to use (major version only)")]
        jdk: u8,
    },
    #[structopt(about = "Update one or all JDKs")]
    Update {
        #[structopt(short, long, help = "Check only, do not download new updates")]
        check: bool,
        #[structopt(help = "The JDK to update (major version only) or 'all'", parse(try_from_str = parse_jdk_or_all))]
        jdk: JdkOrAll,
    },
    #[structopt(about = "List downloaded JDKs")]
    List {},
}

type JdkOrAll = Either<u8, ()>;

fn parse_jdk_or_all(s: &str) -> Result<JdkOrAll, String> {
    s.parse::<u8>().map(Either::Left).or_else(|_| {
        if s == "all" {
            Ok(Either::Right(()))
        } else {
            Err("Not either an JDK major version or 'all'".to_string())
        }
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Jpre::from_args();
    stderrlog::new().verbosity(args.verbose).init()?;
    match args.cmd {
        Subcommand::Use { jdk } => {
            let path = jdk_manager::get_jdk_path(jdk)?;
            println!("JAVA_HOME={}", path.canonicalize()?.display())
        }
        Subcommand::Update { check, jdk } => {
            let majors = jdk.either(
                |jdk_int| Ok(vec![jdk_int]),
                |_| jdk_manager::get_all_jdk_majors(),
            )?;
            let versions = jdk_manager::map_available_jdk_versions(majors);
            let mut update_versions = Vec::new();

            for (major, version) in versions {
                let latest = adoptjdk::get_latest_jdk_version(major)?;
                if latest != version {
                    println!("Update available: {} -> {}", version, latest);
                    update_versions.push(major);
                }
            }

            if !check {
                for major in update_versions {
                    jdk_manager::update_jdk(major)?;
                }
            }
        }
        Subcommand::List {} => {
            let majors = jdk_manager::get_all_jdk_majors()?;
            let versions = jdk_manager::map_available_jdk_versions(majors);
            for (major, version) in versions {
                println!("{}: {}", major, version);
            }
        }
    };
    Ok(())
}
