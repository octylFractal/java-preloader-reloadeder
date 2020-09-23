#![deny(warnings)]
#[macro_use]
extern crate lazy_static;

mod jdk_manager;
mod content_disposition_parser;

use structopt::StructOpt;
use crate::jdk_manager::get_jdk_path;
use std::error::Error;

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
        jdk: u8
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Jpre::from_args();
    stderrlog::new()
        .verbosity(args.verbose)
        .init()?;
    match args.cmd {
        Subcommand::Use { jdk } => {
            let path = get_jdk_path(jdk)?;
            println!(
                "JAVA_HOME={}",
                path.canonicalize()?.display()
            )
        }
    };
    Ok(())
}
