use ::viva::*;
use anyhow::Result;
use clap::{arg, Arg, ArgAction, Command};
use tracing::debug;
use tracing_subscriber::{filter::LevelFilter, util::SubscriberInitExt, EnvFilter};
use config::{Config, Environment, File, FileFormat};
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use directories::ProjectDirs;
use std::fs;
use clap::builder::OsStr;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize, Serialize)]
struct VivaConfig {
    pub default_channels: Vec<String>,
}

fn create_command(viva_config: &VivaConfig) -> Command {

    let verbose_arg = arg!(-v --verbose "Log verbose");
    let default_channels = viva_config.default_channels.iter().map(|s| OsStr::from(s)).collect::<Vec<OsStr>>();

    let environment_arg = Arg::new("env").short('e').long("env").help("The name of the environment to use.").default_value("default");
    let channels_arg = Arg::new("channels").short('c').long("channel").action(ArgAction::Append).help("The channels to use in the environment (if not created yet).").default_values(default_channels);
    let pks_specs_arg = Arg::new("pkg_specs").short('s').long("spec").action(ArgAction::Append).help("The package specs for the environment.");

    let apply_subcommand = Command::new("apply")
        .about("Make sure an environment exists, create it with the provided channels/specs if not.")
        .arg(environment_arg.clone())
        .arg(channels_arg.clone())
        .arg(pks_specs_arg.clone());

    let cmd_arg = Arg::new("cmd").required(true);
    let args_arg = Arg::new("args").action(ArgAction::Append);

    let run_subcommand = Command::new("run")
        .about("Start an executable contained in an environment, create the environment if it doesn't exist.")
        .arg(environment_arg.clone())
        .arg(channels_arg.clone())
        .arg(pks_specs_arg.clone())
        .arg(cmd_arg)
        .arg(args_arg);

    let list_subcommand = Command::new("list")
        .about("List all registered environments.");

    let app = Command::new("viva")
        .version("0.0.4")
        .author("Markus Binsteiner")
        .about("A tool to manage environments and run commands in them.")
        .arg(verbose_arg)
        .subcommand(list_subcommand)
        .subcommand(apply_subcommand)
        .subcommand(run_subcommand);

    app
}

async fn get_config(config_file: &PathBuf) -> Result<Config> {
    let config = Config::builder()
        .add_source(config::File::new(config_file.to_str().unwrap(), FileFormat::Yaml).required(false))
        .add_source(Environment::with_prefix("VIVA"))
        .build()?;
    Ok(config)
}

fn extract_env_spec(matches: &clap::ArgMatches) -> Result<VivaEnvSpec> {
    let channels = match matches.get_many::<String>("channels") {
        Some(channels) => channels.map(|s| s.to_string()).collect::<Vec<String>>(),
        None => vec![],
    };
    let pkg_specs = match matches.get_many::<String>("pkg_specs") {
        Some(pkg_specs) => pkg_specs.map(|s| s.to_string()).collect::<Vec<String>>(),
        None => vec![],
    };
    let env_spec = VivaEnvSpec {
        channels,
        pkg_specs,
    };
    Ok(env_spec)
}

#[tokio::main]
async fn main() -> Result<()> {

    let mut context = viva::VivaContext::init();

    let config_file = context.project_dirs.config_dir().join("viva.yaml");

    if !config_file.exists() {
        let parent = config_file.parent().expect("Could not get parent dir of config file.");
        fs::create_dir_all(parent)?;
        let default_config = "---\ndefault_channels:\n  - conda-forge\n";
        fs::write(&config_file, default_config)?;
    }

    let config_data = get_config(&config_file).await?;
    let viva_config: VivaConfig = config_data.try_deserialize()?;

    let app = create_command(&viva_config);
    let matches = app.get_matches();

    let mut env_collection = Box::new(DefaultEnvCollection::create(&context).await?);
    context.add_env_collection("default", env_collection);

    match matches.subcommand() {
        Some(("apply", apply_matches)) => {
            let env_name = apply_matches.get_one::<String>("env").map(|s| s.to_string()).expect("No environment name provided.");
            let viva_env_spec = extract_env_spec(apply_matches)?;

            let existing_env: &VivaEnv = match context.has_env(&env_name).await {
                true => {
                    context.get_env(&env_name).await?
                }
                false => {
                    // this actually writes an empty spec config file
                    context.add_env(&env_name, None, None).await?
                }
            };

            let env = context.get_env_mut(&env_name).await?;
            if ! viva_env_spec.channels.is_empty() {
                env.add_channels(&viva_env_spec.channels)?;
            }

            if ! viva_env_spec.pkg_specs.is_empty() {
                env.add_pkg_specs(&viva_env_spec.pkg_specs)?;
            }

            env.apply(true).await?;

            println!("apply: {}, {:?}", env_name, env);

        },

        Some(("run", run_matches)) => {
            let env_name = run_matches.get_one::<String>("env").map(|s| s.to_string()).expect("No environment name provided.");
            let viva_env_spec = extract_env_spec(run_matches)?;

            println!("run");
        }
        Some(("list", list_matches)) => {
            println!("list");
            context.pretty_print_envs().await;

        }
        _ => {
            println!("no subcommand");
        }
    }


    // println!("{:?}", matches);
    // let args = Cli::parse();
    //
    // let env: String = match args.env {
    //     Some(env_name) => env_name,
    //     None => "default".to_string(),
    // };
    //
    // // Determine the logging level based on the the verbose flag and the RUST_LOG environment
    // // variable.
    // let default_filter = if args.verbose {
    //     LevelFilter::DEBUG
    // } else {
    //     LevelFilter::INFO
    // };
    // let env_filter = EnvFilter::builder()
    //     .with_default_directive(default_filter.into())
    //     .from_env()
    //     .expect("Failed to parse the RUST_LOG environment variable")
    //     // filter logs from apple codesign because they are very noisy
    //     .add_directive(
    //         "apple_codesign=off"
    //             .parse()
    //             .expect("Failed to parse the RUST_LOG environment variable"),
    //     );
    //
    // // Setup the tracing subscriber
    // tracing_subscriber::fmt()
    //     .with_env_filter(env_filter)
    //     .with_writer(IndicatifWriter::new(global_multi_progress()))
    //     .without_time()
    //     .finish()
    //     .try_init()
    //     .expect("Failed to initialize the tracing subscriber");
    //
    // let viva_env = context.get_env(&env).await?;
    //
    // debug!("Starting viva with args: {:?}", &args.command);
    // match args.command {
    //     Action::Apply {} => handle_result(viva_env.apply().await),
    //     Action::Run(cmd_args) => {
    //         let apply_result = viva_env.apply().await;
    //         if let Err(e) = apply_result {
    //             eprintln!("Error: {}", e);
    //             for cause in e.chain().skip(1) {
    //                 eprintln!("Caused by: {}", cause);
    //             }
    //             std::process::exit(1);
    //         }
    //         handle_result(
    //             viva_env
    //                 .run_command_in_env(&cmd_args.cmd)
    //                 .await,
    //         )
    //     }
    //     Action::ListEnvs => {
    //         context.pretty_print_envs().await;
    //     }
    //     Action::Remove => {
    //         handle_result(viva_env.remove().await);
    //     }
    // }

    Ok(())
}
