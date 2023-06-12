use ::viva::*;
use anyhow::{bail, Result};
use clap::builder::OsStr;
use clap::{arg, Arg, ArgAction, Command};
use config::{Config, Environment, FileFormat};

use serde::{Deserialize, Serialize};


use std::fs;
use std::path::{PathBuf};
use tracing::debug;
// use tracing_subscriber::{util::SubscriberInitExt};
use viva::models::app::{AppEnvPlacementStrategy, DefaultAppCollection, VivaAppSpec};
use viva::models::environment::DefaultEnvCollection;

// fn handle_result<T>(result: Result<T, anyhow::Error>) -> T {
//     if let Err(e) = result {
//         eprintln!("Error: {}", e);
//         for cause in e.chain().skip(1) {
//             eprintln!("Caused by: {}", cause);
//         }
//         std::process::exit(1);
//     } else {
//         result.unwrap()
//     }
// }

#[derive(Debug, Deserialize, Serialize)]
struct VivaConfig {
    pub default_channels: Vec<String>,
}

fn create_command(viva_config: &VivaConfig) -> Command {
    let verbose_arg = arg!(-v --verbose "Log verbose");
    let default_channels = viva_config
        .default_channels
        .iter()
        .map(|s| OsStr::from(s))
        .collect::<Vec<OsStr>>();

    let environment_arg = Arg::new("env")
        .help("The name of the environment to use.")
        .default_value("default");
    let replace_arg = Arg::new("replace")
        .action(ArgAction::SetTrue)
        .short('r')
        .long("replace")
        .help("Replace the environment if it already exists.");

    let channels_arg = Arg::new("channels")
        .short('c')
        .long("channel")
        .action(ArgAction::Append)
        .help("The required channels.")
        .default_values(default_channels);

    let pks_specs_arg = Arg::new("pkg_specs")
        .short('s')
        .long("spec")
        .action(ArgAction::Append)
        .help("The required package specs.");

    let app_name = Arg::new("app")
        .help("The name to register the application.")
        .required(true);

    let executable_arg = Arg::new("executable")
        .short('e')
        .long("executable")
        .required(false)
        .help("The name of the executable to run (defaults to the app name).");

    let app_args = Arg::new("cmd_args")
        .long("arg")
        .action(ArgAction::Append)
        .help("The (optional) arguments for the command to run.")
        .default_values(Vec::<OsStr>::new());

    let env_sync = Arg::new("sync")
        .action(ArgAction::SetTrue)
        .short('S')
        .long("sync")
        .help("Install all environment packages locally, now.")
        ;

    let app_sync = Arg::new("sync")
        .action(ArgAction::SetTrue)
        .short('S')
        .long("sync")
        .help("Install all required packages locally, now.")
        ;

    let register_env_subcommand = Command::new("register-env")
        .about(
            "Register an environment, and optionally create it locally.",
        )
        .arg(environment_arg.clone())
        .arg(channels_arg.clone())
        .arg(pks_specs_arg.clone())
        .arg(replace_arg)
        .arg(env_sync);

    let delete_env_subcommand = Command::new("delete-env")
        .about("Delete an environment.")
        .arg(environment_arg.clone());

    let register_app_subcommand = Command::new("register-app")
        .about("Register an app, and optionally install all the required packages locally.")
        .arg(app_name)
        .arg(channels_arg.clone())
        .arg(pks_specs_arg.clone())
        .arg(executable_arg)
        .arg(app_args)
        .arg(app_sync);


    let cmd_arg = Arg::new("cmd").required(true).help("The command to run.");
    let cmd_args = Arg::new("cmd_args").action(ArgAction::Append).help("The (optional) arguments for the command to run.").default_values(Vec::<OsStr>::new());

    let run_subcommand = Command::new("run")
        .about("Start an executable contained in an environment, create the environment if it doesn't exist.")
        .arg(environment_arg.clone())
        .arg(channels_arg.clone())
        .arg(pks_specs_arg.clone())
        .arg(cmd_arg)
        .arg(cmd_args);

    let list_envs_subcommand = Command::new("list-envs").about("List all registered environments.");

    let list_apps_subcommand = Command::new("list-apps").about("List all registered apps.");

    let app = Command::new("viva")
        .version("0.0.4")
        .author("Markus Binsteiner")
        .about("A tool to manage environments and run commands in them.")
        .arg(verbose_arg)
        .subcommand(list_envs_subcommand)
        .subcommand(register_env_subcommand)
        .subcommand(delete_env_subcommand)
        .subcommand(list_apps_subcommand)
        .subcommand(register_app_subcommand)
        .subcommand(run_subcommand);

    app
}

async fn get_config(config_file: &PathBuf) -> Result<Config> {
    let config = Config::builder()
        .add_source(
            config::File::new(config_file.to_str().unwrap(), FileFormat::Yaml).required(false),
        )
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
        let parent = config_file
            .parent()
            .expect("Could not get parent dir of config file.");
        fs::create_dir_all(parent)?;
        let default_config = "---\ndefault_channels:\n  - conda-forge\n";
        fs::write(&config_file, default_config)?;
    }

    let config_data = get_config(&config_file).await?;
    let viva_config: VivaConfig = config_data.try_deserialize()?;

    let app = create_command(&viva_config);
    let matches = app.get_matches();

    // let env_base_path = context.project_dirs.data_dir().join("envs");
    let config_path = PathBuf::from(context.project_dirs.config_dir());

    let env_collection =
        Box::new(DefaultEnvCollection::create(config_path.clone()).await?);
    context
        .add_env_collection("default", env_collection)
        .await?;

    let placement_strategy = AppEnvPlacementStrategy::CollectionId;

    let app_collection = Box::new(DefaultAppCollection::create(config_path).await?);
    context.add_app_collection("default", app_collection, Some(placement_strategy)).await?;

    match matches.subcommand() {
        Some(("register-env", apply_matches)) => {
            debug!("running 'apply' subcommand");
            let env_name = apply_matches
                .get_one::<String>("env")
                .map(|s| s.to_string())
                .expect("No environment name provided.");
            let viva_env_spec = extract_env_spec(apply_matches)?;

            match context.has_env(&env_name).await {
                true => {
                    let replace = apply_matches.get_flag("replace");
                    if replace {
                        context.remove_env(&env_name).await?;
                    } else {
                        bail!("environment {} already registered", env_name);
                    }
                    debug!("environment {} already registered", env_name);
                    // context.get_env(&env_name).await?
                }
                false => {
                    // this actually writes an empty spec config file
                    context.add_env(&env_name, None, None).await?;
                }
            };

            context
                .merge_env_specs(&env_name, &viva_env_spec, true, true)
                .await?;

            let sync = apply_matches.get_flag("sync");
            if sync {
                let env = context.get_env_mut(&env_name).await?;
                env.apply().await?;
                println!("Registered and applied environment: {}", env_name);
            } else {
                // let env = context.get_env(&env_name).await?;
                println!("Registered environment: {}", env_name);
            }


        }
        Some(("delete-env", delete_matches)) => {
            debug!("running 'delete' subcommand");
            let env_name = delete_matches
                .get_one::<String>("env")
                .map(|s| s.to_string())
                .expect("No environment name provided.");
            context.remove_env(&env_name).await?;
            println!("Deleted environment: {}", env_name);
        }
        Some(("list-envs", _list_matches)) => {
            debug!("running 'run' subcommand");
            context.check_envs_sync_status().await?;
            context.pretty_print_envs().await;
        }
        Some(("list-apps", _app_matches)) => {
            debug!("running 'run' subcommand");
            context.merge_all_apps().await?;
            context.check_envs_sync_status().await?;
            context.pretty_print_apps().await;
        }
        Some(("register-app", set_app_matches)) => {
            debug!("running 'set-app' subcommand");
            let app_id = set_app_matches
                .get_one::<String>("app")
                .map(|s| s.to_string())
                .expect("No app name provided.");

            let viva_env_spec = extract_env_spec(set_app_matches)?;
            let executable = set_app_matches
                .get_one::<String>("executable")
                .map(|s| s.to_string());


            let args = match set_app_matches.get_many::<String>("cmd_args") {
                Some(cmd_args) => cmd_args.map(|s| s.to_string()).collect::<Vec<String>>(),
                None => vec![],
            };

            let exe = match executable {
                Some(exe) => exe,
                None => {
                    debug!("No executable provided, using app name as executable.");
                    app_id.clone()
                }
            };

            let app_spec = VivaAppSpec {
                executable: exe,
                args,
                env_spec: viva_env_spec,
            };

            println!("set-app: {}", app_id);
            println!("{:?}", &app_spec);

            let col_id = "default";
            let placement_strategy = AppEnvPlacementStrategy::CollectionId;

            context.add_app(&app_id, app_spec, col_id, placement_strategy).await?;


        }
        Some(("run", run_matches)) => {
            debug!("running 'run' subcommand");
            let _env_name = run_matches
                .get_one::<String>("env")
                .map(|s| s.to_string())
                .expect("No environment name provided.");
            let _viva_env_spec = extract_env_spec(run_matches)?;

            println!("run");
        }

        _ => {
            println!("No subcommand provided, use the '--help' flag to get more information.)");
        }
    }

    Ok(())
}
