use ::viva::*;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::debug;
use tracing_subscriber::{filter::LevelFilter, util::SubscriberInitExt, EnvFilter};

#[derive(Parser, Debug)]
struct Cli {
    /// The name of the environment to use, default: 'default'.
    #[clap(short, long, global = true)]
    env: Option<String>,
    /// The strategy to use when checking for the environment, allowed values are: auto, force, skip, default: 'auto'.
    #[clap(long, short = 'l', global = true)]
    env_load_strategy: Option<EnvLoadStrategy>,
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
pub struct CmdSpec {}

#[derive(Debug, clap::Parser)]
pub struct RunCmdSpec {
    /// The command and its arguments to run in the environment
    #[clap(last = true)]
    pub cmd: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Make sure an environment exists, create it with the provided channels/specs if not.
    Apply {},
    /// Start an executable contained in an environment, create the environment if it doesn't exist.
    Run(RunCmdSpec),
    /// List all available environments.
    ListEnvs,
    Remove
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
    let globals = viva::VivaGlobals::new();

    let env: String = match args.env {
        Some(env_name) => env_name,
        None => "default".to_string(),
    };

    let load_strategy = match args.env_load_strategy {
        Some(strategy) => strategy,
        None => EnvLoadStrategy::Merge,
    };

    // Determine the logging level based on the the verbose flag and the RUST_LOG environment
    // variable.
    let default_filter = if args.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    let env_filter = EnvFilter::builder()
        .with_default_directive(default_filter.into())
        .from_env()
        .expect("Failed to parse the RUST_LOG environment variable")
        // filter logs from apple codesign because they are very noisy
        .add_directive(
            "apple_codesign=off"
                .parse()
                .expect("Failed to parse the RUST_LOG environment variable"),
        );

    // Setup the tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(IndicatifWriter::new(global_multi_progress()))
        .without_time()
        .finish()
        .try_init()
        .expect("Failed to initialize the tracing subscriber");

    let viva_env_status = handle_result(
        VivaEnvStatus::init_env(&env, args.specs, args.channels, load_strategy, &globals).await,
    );

    debug!("Starting viva with args: {:?}", &args.command);
    match args.command {
        Action::Apply {} => handle_result(viva_env_status.apply().await),
        Action::Run(cmd_args) => {
            let apply_result = viva_env_status.apply().await;
            if let Err(e) = apply_result {
                eprintln!("Error: {}", e);
                for cause in e.chain().skip(1) {
                    eprintln!("Caused by: {}", cause);
                }
                std::process::exit(1);
            }
            handle_result(
                viva_env_status
                    .viva_env
                    .run_command_in_env(&cmd_args.cmd)
                    .await,
            )
        }
        Action::ListEnvs => {
            globals.pretty_print_envs().await;
        }
        Action::Remove => {
            handle_result(viva_env_status.viva_env.remove().await);
        }
    }
}
