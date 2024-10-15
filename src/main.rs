use crate::command::current::Current;
use crate::command::debug::Debug;
use crate::command::get_context_id::GetContextId;
use crate::command::java_home::JavaHome;
use crate::command::list_distributions::ListDistributions;
use crate::command::list_installed::ListInstalled;
use crate::command::list_versions::ListVersions;
use crate::command::remove_jdk::RemoveJdk;
use crate::command::set_distribution::SetDistribution;
use crate::command::update::UpdateInstalled;
use crate::command::use_jdk::UseJdk;
use crate::command::{Context, JpreCommand};
use crate::config::JpreConfig;
use crate::error::{ESResult, JpreError, UserMessage};
use clap::{Parser, Subcommand};
use enum_dispatch::enum_dispatch;
use tracing::error;
use tracing_subscriber::fmt::format::{DefaultFields, Format};
use tracing_subscriber::fmt::FormatEvent;
use tracing_subscriber::Registry;

#[cfg(not(unix))]
compile_error!("Only unix is supported");

mod checksum_verifier;
mod command;
mod config;
mod context_id;
mod error;
mod foojay;
mod http_client;
mod java_home_management;
mod java_version;
mod jdk_manager;
mod progress;
mod string;

/// java-preloader-reloadeder. A tool to manage Java installations.
#[derive(Debug, Parser)]
struct Jpre {
    #[clap(subcommand)]
    command: JpreCommandEnum,
    /// Verbosity level, repeat to increase.
    #[clap(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, Subcommand)]
#[enum_dispatch(JpreCommand)]
enum JpreCommandEnum {
    ListDistributions(ListDistributions),
    ListVersions(ListVersions),
    ListInstalled(ListInstalled),
    SetDistribution(SetDistribution),
    Debug(Debug),
    Use(UseJdk),
    Remove(RemoveJdk),
    GetContextId(GetContextId),
    JavaHome(JavaHome),
    Current(Current),
    Update(UpdateInstalled),
}

fn main() {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        error!("Unsupported system: {}", std::env::consts::OS);
        std::process::exit(1);
    }

    match main_with_result() {
        Ok(()) => (),
        Err(e) if matches!(e.current_context(), JpreError::UserError) => {
            if !e.contains::<UserMessage>() {
                error!("Critical error, user error missing message:\n{:?}", e);
                std::process::exit(2);
            }
            error!("Error in user input:");
            for m in e
                .frames()
                .filter_map(|f| f.downcast_ref::<UserMessage>())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                error!("  {}", m.message);
            }
            std::process::exit(1);
        }
        Err(e) => {
            error!("{:?}", e);
            std::process::exit(2);
        }
    }
}

fn main_with_result() -> ESResult<(), JpreError> {
    let config = JpreConfig::load()?;
    // re-save config to ensure it's up-to-date
    config.save()?;
    let args = Jpre::parse();

    let env_filt = tracing_subscriber::filter::EnvFilter::builder()
        .with_default_directive(
            match args.verbose {
                0 => tracing_subscriber::filter::LevelFilter::INFO,
                1 => tracing_subscriber::filter::LevelFilter::DEBUG,
                _ => tracing_subscriber::filter::LevelFilter::TRACE,
            }
            .into(),
        )
        .from_env_lossy()
        // Set some loud things to warn
        .add_directive("reqwest=warn".parse().unwrap())
        .add_directive("hyper=warn".parse().unwrap());

    fn install_with_event_format<E>(format: E, env_filt: tracing_subscriber::filter::EnvFilter)
    where
        E: FormatEvent<Registry, DefaultFields> + Send + Sync + 'static,
    {
        tracing_subscriber::fmt()
            .event_format(format)
            .with_env_filter(env_filt)
            .init();
    }
    if args.verbose == 0 {
        install_with_event_format(
            Format::default()
                .compact()
                .without_time()
                .with_target(false),
            env_filt,
        );
    } else {
        install_with_event_format(Format::default(), env_filt);
    }

    let context = Context {
        config: config.clone(),
    };

    args.command.run(context)
}
