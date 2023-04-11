use clap::{Parser, Subcommand};
use tracing_subscriber::{EnvFilter, filter::LevelFilter, util::SubscriberInitExt};

use environment::VivaEnv;
use crate::environment::{EnvCheckStrategy, PkgInstallStrategy};

use crate::rattler::global_multi_progress;
use crate::rattler::writer::IndicatifWriter;

mod defaults;
mod errors;
mod micromamba;
mod rattler;
mod status;
mod environment;


#[derive(Parser, Debug)]
struct Cli {
    /// The name of the environment to use, default: 'default'.
    #[clap(short, long, global = true)]
    env: Option<String>,
    /// The strategy to use when checking for the environment, allowed values are: auto, force, skip, default: 'auto'.
    #[clap(long, short='C', global = true)]
    env_check_strategy: Option<EnvCheckStrategy>,
    /// The channels to use in the environment (if not created yet), default: 'conda-forge'.
    #[clap(short, long, global = true)]
    channels: Option<Vec<String>>,
    /// The specs to install into the environment.
    #[clap(short, long, global = true)]
    specs: Option<Vec<String>>,

    /// Log verbose
    #[clap(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Action,


}

#[derive(Debug, clap::Parser)]
pub struct CmdSpec {

}

#[derive(Debug, clap::Parser)]
pub struct RunCmdSpec {
    /// The command and its arguments to run in the environment
    #[clap(last = true)]
    pub cmd: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Ensure an environment exists, create it with the provided channels/specs if not.
    Ensure {
    },
    /// Start an executable contained in an environment, create the environment if it doesn't exist.
    Run(RunCmdSpec),
}

fn handle_result<T>(result: Result<T, anyhow::Error>) -> T {
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        for cause in e.chain().skip(1) {
            eprintln!("Caused by: {}", cause);
        }
        std::process::exit(1);
    } else {
        result.unwrap()
    }
}

#[tokio::main]
async fn main() {

    let args = Cli::parse();
    let globals = defaults::Globals::new();

    let viva_env = handle_result(VivaEnv::create(args.env.as_deref().unwrap_or("default"), args.specs, args.channels, &globals));

    // Determine the logging level based on the the verbose flag and the RUST_LOG environment
    // variable.
    let default_filter = if args.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    let env_filter = EnvFilter::builder()
        .with_default_directive(default_filter.into())
        .from_env().expect("Failed to parse the RUST_LOG environment variable")
        // filter logs from apple codesign because they are very noisy
        .add_directive("apple_codesign=off".parse().expect("Failed to parse the RUST_LOG environment variable"));

    // Setup the tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(IndicatifWriter::new(global_multi_progress()))
        .without_time()
        .finish()
        .try_init().expect("Failed to initialize the tracing subscriber");

    let check_strategy = args.env_check_strategy.unwrap_or(EnvCheckStrategy::Auto);
    let pkg_install_strategy = PkgInstallStrategy::Append;

    match args.command {
        Action::Ensure { } => handle_result(viva_env.ensure(check_strategy, pkg_install_strategy).await),
        Action::Run(cmd_args) => handle_result(viva_env.run_command_in_env(&cmd_args.cmd, check_strategy).await),
    }

}


