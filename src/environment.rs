use std::collections::HashSet;
use std::path::{PathBuf, MAIN_SEPARATOR};
use std::process::Stdio;
use std::str::FromStr;

use anyhow::{anyhow, Context, Error, Result};
use rattler_repodata_gateway::fetch::CacheAction;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Result as SerdeJsonResult;
use serde_yaml::Result as SerdeYamlResult;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::debug;

use crate::defaults::{CONDA_BIN_DIRNAME, ENV_SPEC_FILENAME};
use crate::VivaGlobals;

#[derive(Debug, Clone)]
pub enum EnvLoadStrategy {
    Force,
    Merge,
    New,
}

#[derive(Debug, Clone)]
pub enum EnvLoadAction {
    Create,
    Merge,
    Overwrite,
}

/// Allows converting a string to an `EnvLoadStrategy`.
impl FromStr for EnvLoadStrategy {
    type Err = Error;

    /// Creates an `EnvLoadStrategy` from the given string.
    ///
    /// # Arguments
    ///
    /// * `s` - A string that represents the environment check strategy.
    ///
    /// # Errors
    ///
    /// Returns an error if the given string does not match any of the available strategies.

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "force" => Ok(EnvLoadStrategy::Force),
            "new" => Ok(EnvLoadStrategy::New),
            "merge" => Ok(EnvLoadStrategy::Merge),
            _ => Err(anyhow!(
                "Invalid environment environment load strategy: {}. Available: new, merge, force",
                s
            )),
        }
    }
}

/// Represents the Viva environment specification.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VivaEnv {
    pub target_prefix: PathBuf,
    pub channels: Vec<String>,
    pub specs: Vec<String>,
    pub env_spec_file: PathBuf,
}

#[derive(Debug)]
pub struct VivaEnvPaths {
    target_prefix: PathBuf,
    target_prefix_exists: bool,
    env_spec_file: PathBuf,
    env_spec_file_exists: bool,
}

#[derive(Debug)]
pub struct VivaEnvStatus {
    pub viva_env: VivaEnv,
    dirty: bool,
}

/// Join two matchspecs lists into a single one.
fn join_matchspecs(spec_1: &Vec<String>, spec_2: &Vec<String>) -> Vec<String> {
    let mut specs: HashSet<String> = HashSet::new();
    specs.extend(spec_1.iter().cloned());
    specs.extend(spec_2.iter().cloned());
    return specs.into_iter().collect();
}

/// Join two channel lists into a single one.
fn join_channels(channel_1: &Vec<String>, channel_2: &Vec<String>) -> Vec<String> {
    let mut specs: HashSet<String> = HashSet::new();
    specs.extend(channel_1.iter().cloned());
    specs.extend(channel_2.iter().cloned());
    return specs.into_iter().collect();
}

fn matchspecs_are_equal(spec_1: &Vec<String>, spec_2: &Vec<String>) -> bool {
    let mut specs_1: HashSet<String> = HashSet::new();
    specs_1.extend(spec_1.iter().cloned());
    let mut specs_2: HashSet<String> = HashSet::new();
    specs_2.extend(spec_2.iter().cloned());
    return specs_1 == specs_2;
}

fn check_for_new_matchspecs(
    orig_matchspec: &Vec<String>,
    new_matchspec: &Vec<String>,
) -> Vec<String> {
    let mut result = Vec::new();

    for spec in new_matchspec {
        if !orig_matchspec.contains(spec) {
            result.push(spec.clone());
        }
    }
    return result;
}

fn check_for_new_channels(orig_channels: &Vec<String>, new_channels: &Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for channel in new_channels {
        if !orig_channels.contains(channel) {
            result.push(channel.clone());
        }
    }
    return result;
}

fn channels_are_equal(channel_1: &Vec<String>, channel_2: &Vec<String>) -> bool {
    let mut channels_1: HashSet<String> = HashSet::new();
    channels_1.extend(channel_1.iter().cloned());
    let mut channels_2: HashSet<String> = HashSet::new();
    channels_2.extend(channel_2.iter().cloned());
    return channels_1 == channels_2;
}

impl VivaEnvStatus {
    /// Returns a `VivaEnvStatus` if the environment configuration is successfully created, or an error if there is a problem.
    pub async fn init_env<S: AsRef<str>, I: AsRef<[S]>>(
        env: &str,
        specs: Option<I>,
        channels: Option<I>,
        load_strategy: EnvLoadStrategy,
        globals: &VivaGlobals,
    ) -> anyhow::Result<VivaEnvStatus> {
        let paths = VivaEnv::resolve_paths(env, globals).await?;

        debug!("Resolved paths: {:?}", paths);

        let mut action: EnvLoadAction;

        if !paths.env_spec_file_exists {
            action = EnvLoadAction::Create;
        } else {
            match load_strategy {
                EnvLoadStrategy::Force => {
                    action = EnvLoadAction::Overwrite;
                }
                EnvLoadStrategy::Merge => {
                    action = EnvLoadAction::Merge;
                }
                EnvLoadStrategy::New => {
                    if paths.target_prefix_exists {
                        return Err(anyhow!(
                            "Environment prefix already exists: {}",
                            paths.target_prefix.display()
                        ));
                    }
                    if paths.env_spec_file_exists {
                        return Err(anyhow!(
                            "Environment specification file already exists: {}",
                            paths.env_spec_file.display()
                        ));
                    }
                    // this will never happen
                    action = EnvLoadAction::Create;
                }
            }
        }

        let mut new_env_specs: Vec<String> = if let Some(s) = specs {
            s.as_ref()
                .iter()
                .map(|x| String::from(x.as_ref()))
                .collect()
        } else {
            vec![]
        };
        let mut new_env_channels: Vec<String> = if let Some(c) = channels {
            c.as_ref()
                .iter()
                .map(|x| String::from(x.as_ref()))
                .collect()
        } else {
            vec![]
        };

        let env_folder_missing = !paths.target_prefix_exists && paths.env_spec_file_exists;

        let final_env_specs: Vec<String>;
        let final_channels: Vec<String>;
        let mut change_required: bool = false;

        if env_folder_missing {
            debug!("Environment prefix does not exist, but env spec file does. Setting 'dirty' to 'true', and adding all (old) metadata to new env spec file.");
            change_required = true;
            let existing_env = VivaEnv::read_env_spec(&paths.env_spec_file).await?;
            new_env_specs = join_matchspecs(&existing_env.specs, &new_env_specs);
            new_env_channels = join_channels(&existing_env.channels, &new_env_channels);
            action = EnvLoadAction::Create;
        }

        debug!("Registering env action '{:?}' for: {:?}", action, env);

        match action {
            EnvLoadAction::Create => {
                final_env_specs = new_env_specs;
                final_channels = new_env_channels;
                change_required = true;
            }
            EnvLoadAction::Overwrite => {
                let existing_env = VivaEnv::read_env_spec(&paths.env_spec_file).await?;
                if channels_are_equal(&new_env_channels, &existing_env.channels)
                    && matchspecs_are_equal(&new_env_specs, &existing_env.specs)
                {
                    debug!("No need to overwrite environment specs, old and new specs are equal.");
                } else {
                    debug!("Overwriting environment specs, old and new specs are not equal.");
                    change_required = true;
                }
                final_env_specs = new_env_specs;
                final_channels = new_env_channels;
            }
            EnvLoadAction::Merge => {
                let existing_env = VivaEnv::read_env_spec(&paths.env_spec_file).await?;

                let new_env_specs: Vec<String> =
                    check_for_new_matchspecs(&existing_env.specs, &new_env_specs);

                match new_env_specs.len() {
                    0 => {
                        debug!("No need to merge environment specs, old env contains all items from new specs.");
                        final_env_specs = new_env_specs;
                    }
                    _ => {
                        debug!("Merging environment specs, new env contains items not in old specs: {}", new_env_specs.join(", "));
                        change_required = true;
                        final_env_specs = join_matchspecs(&new_env_specs, &existing_env.specs);
                    }
                }
                let new_channels: Vec<String> =
                    check_for_new_channels(&existing_env.channels, &new_env_channels);
                match new_channels.len() {
                    0 => {
                        debug!("No need to merge environment channels, new env does not have any new channels.");
                        final_channels = new_env_channels;
                    }
                    _ => {
                        debug!("Merging environment channels, new env contains items not in old channels: {}", new_channels.join(", "));
                        change_required = true;
                        final_channels = join_channels(&existing_env.channels, &new_env_channels);
                    }
                }
            }
        }

        debug!(
            "Environment '{:?}' 'dirty' status: {:?}",
            env, change_required
        );

        let env_spec = VivaEnv {
            target_prefix: paths.target_prefix,
            channels: final_channels,
            specs: final_env_specs,
            env_spec_file: paths.env_spec_file,
        };

        let env_spec_status = VivaEnvStatus {
            viva_env: env_spec,
            dirty: change_required,
        };

        Ok(env_spec_status)
    }

    pub async fn apply(&self) -> anyhow::Result<()> {
        if self.dirty {
            self.viva_env.apply().await?;
        }
        Ok(())
    }
}

impl VivaEnv {
    /// Constructs a `VivaEnv` from the given parameters.
    ///
    /// # Arguments
    ///
    /// * `env` - The name or path to or specs of the environment.
    /// * `specs` - An optional list of specs for the environment.
    /// * `channels` - An optional list of channels for the environment.
    /// * `globals` - A reference to a `Globals` object.
    ///
    /// # Returns

    pub async fn resolve_paths(env: &str, globals: &VivaGlobals) -> anyhow::Result<VivaEnvPaths> {
        let valid_alias_chars_re = Regex::new(r"[^A-Za-z0-9_]+").unwrap();

        let env_prefix: PathBuf;
        let env_spec_file: PathBuf;

        // Check if the environment string contain a path separator.
        match env.contains(MAIN_SEPARATOR) {
            true => {
                let path = PathBuf::from(env);

                match path.is_file() {
                    true => {
                        env_prefix = path;
                    }
                    false => {
                        env_prefix = path;
                    }
                }

                env_spec_file = env_prefix.join(ENV_SPEC_FILENAME);
            }
            false => {
                // check if we have any special characters
                match !valid_alias_chars_re.is_match(env) {
                    true => {
                        env_prefix = globals.get_env_data_path(env);
                        env_spec_file = globals.get_env_config_path(env);
                    }
                    false => {
                        // let temp_env = VivaEnv::parse_env_spec(env).await?;
                        // env_prefix = temp_env.target_prefix;
                        // env_spec_file = temp_env.env_spec_file;
                        panic!("String env spec not implemented yet.")
                    }
                }
            }
        };

        let env_exists: bool = match env_prefix.exists() {
            true => {
                if env_prefix.is_file() {
                    return Err(anyhow!(
                        "Environment prefix path is a file: {}",
                        env_prefix.display()
                    ));
                }
                true
            }
            false => false,
        };

        let env_spec_file_exists: bool = match env_spec_file.exists() {
            true => {
                if env_spec_file.is_dir() {
                    return Err(anyhow!(
                        "Environment specification file is a directory: {}",
                        env_spec_file.display()
                    ));
                }
                true
            }
            false => false,
        };

        if env_exists && !env_spec_file_exists {
            return Err(anyhow!(
                "Environment prefix exists but specification file does not: {}",
                env_prefix.display()
            ));
        }

        let result = VivaEnvPaths {
            target_prefix: env_prefix,
            target_prefix_exists: env_exists,
            env_spec_file,
            env_spec_file_exists,
        };
        Ok(result)
    }

    pub async fn load(env: &str, globals: &VivaGlobals) -> Result<VivaEnv> {
        let paths = VivaEnv::resolve_paths(env, globals).await?;
        let env_spec = VivaEnv::read_env_spec(&paths.env_spec_file).await?;

        Ok(env_spec)
    }

    /// Read env spec data from a file.
    pub(crate) async fn read_env_spec(env_spec_file: &PathBuf) -> Result<VivaEnv> {
        let mut file = File::open(env_spec_file).await?;
        let mut env_spec_data = String::new();
        file.read_to_string(&mut env_spec_data).await?;

        match VivaEnv::parse_env_spec(&env_spec_data).await {
            Ok(env_spec) => {
                return Ok(env_spec);
            }
            Err(_) => {
                return Err(anyhow!(
                    "Unable to parse environment specification file: {}",
                    env_spec_file.display()
                ));
            }
        }
    }

    pub(crate) async fn parse_env_spec(env_spec_data: &str) -> Result<VivaEnv> {
        let json_result: SerdeJsonResult<VivaEnv> = serde_json::from_str(&env_spec_data);
        match json_result {
            Ok(env_spec) => {
                return Ok(env_spec);
            }
            Err(_) => {
                let yaml_result: SerdeYamlResult<VivaEnv> = serde_yaml::from_str(&env_spec_data);
                return yaml_result.with_context(|| {
                    format!(
                        "Unable to parse environment specification string: {}",
                        env_spec_data
                    )
                });
            }
        }
    }

    pub(crate) async fn write_env_spec(&self) -> anyhow::Result<()> {
        let env_spec_file = &self.env_spec_file;

        let env_spec_json = serde_json::to_string(&self).expect(&format!(
            "Cannot serialize environment spec to JSON: {}",
            &env_spec_file.to_string_lossy()
        ));

        if let Some(parent) = env_spec_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        std::fs::write(&env_spec_file, env_spec_json).expect(&format!(
            "Cannot write environment spec to file: {}",
            &env_spec_file.to_string_lossy()
        ));
        Ok(())
    }

    /// Creates a command in the environment, with the specified environment-check  & package-install strategy..
    pub async fn create_command_in_env<S: AsRef<str>, I: AsRef<[S]>>(
        &self,
        cmd: I,
    ) -> Result<Command> {
        let mut iter = cmd.as_ref().iter();
        let executable: &str;
        let rest: Vec<&S>;
        let cmd_args: Vec<&str>;
        if let Some(first) = iter.next() {
            executable = first.as_ref();
            rest = iter.collect();
            cmd_args = rest.iter().map(|s| s.as_ref()).collect();
        } else {
            return Err(anyhow!("No command provided"));
        }
        let mut full_exe_path = self.target_prefix.join(CONDA_BIN_DIRNAME).join(executable);

        let final_exe_path: PathBuf = match full_exe_path.exists() {
            true => full_exe_path,
            false => {
                match full_exe_path.ends_with(".exe") {
                    true => {
                        full_exe_path.set_extension("");
                    }
                    false => {
                        full_exe_path.set_extension("exe");
                    }
                }
                match full_exe_path.exists() {
                    true => full_exe_path,
                    false => {
                        return Err(anyhow!(
                            "Could not find executable (after setup env phase): {}",
                            executable
                        ));
                    }
                }
            }
        };

        let mut command = Command::new(final_exe_path);

        if cmd_args.len() > 0 {
            command.args(cmd_args);
        }

        Ok(command)
    }

    /// Runs a command in the context of the environment, using the specified environment-check & package-install strategy.
    ///
    /// # Arguments
    ///
    /// * `cmd` - A sequence of strings representing the command and its arguments.
    /// * `env_check_strategy` - The strategy to use when checking for the environment.
    /// * `pkg_install_strategy` - The strategy to use when installing packages.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the command runs successfully, or an error if there is a problem.
    pub async fn run_command_in_env<S: AsRef<str>, I: AsRef<[S]>>(&self, cmd: I) -> Result<()> {
        let mut command = self.create_command_in_env(&cmd).await?;

        let child = command.stdout(Stdio::piped()).spawn().expect(
            format!(
                "Failed to spawn subprocess: {}",
                cmd.as_ref()
                    .iter()
                    .map(|s| s.as_ref())
                    .collect::<Vec<&str>>()
                    .join(" ")
            )
            .as_str(),
        );

        let output = child.wait_with_output().await?;
        // unsafe { child.detach() };

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{}", stdout);
        } else {
            eprintln!("{:?}", output);
        }

        Ok(())
    }

    /// Applies the environments specs and ensures it is created and ready to be used.
    ///
    /// Be aware that this could remove some packages that already exist in the environment.
    ///
    /// # Arguments
    ///
    /// * `env_check_strategy` - The strategy to use when checking for the environment.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the environment check is successful, or an error if there is a problem.
    pub async fn apply(&self) -> Result<()> {
        let cache_action = CacheAction::CacheOrFetch;

        debug!("Applying environment: {:?}", &self);

        let create_result = crate::rattler::commands::create::create(&self, cache_action)
            .await
            .with_context(|| format!("Failed to create environment: {:?}", &self));

        debug!("Environment created: {:?}", &create_result);
        match create_result {
            Ok(_) => {
                // TODO: delete created env if this fails?
                self.write_env_spec().await?;
                return Ok(());
            }
            Err(e) => return Err(e),
        }
    }

    pub async fn remove(&self) -> Result<()> {
        let deleted_target_prefix = match &self.target_prefix.exists() {
            true => match fs::remove_dir_all(&self.target_prefix).await {
                Ok(_) => Ok(()),
                Err(e) => Err(anyhow!("Failed to remove environment: {}", e)),
            },
            false => Ok(()),
        };

        match deleted_target_prefix {
            Ok(_) => match &self.env_spec_file.exists() {
                true => match fs::remove_file(&self.env_spec_file).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!("Failed to remove environment spec file: {}", e)),
                },
                false => Ok(()),
            },
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_viva_env_from_str_with_spec_file() {
        // let env_name = "test_env";
        // let globals = VivaGlobals::create("tests", "test_org", "viva", None);
        //
        // let viva_env_orig = VivaEnvStatus::init_env(
        //     env_name,
        //     Some(vec![String::from("spec1"), String::from("spec2")]),
        //     Some(vec![String::from("channel_1"), String::from("channel_2")]),
        //     EnvLoadStrategy::Merge,
        //     &globals,
        // )
        // .await.unwrap();
        // let env_spec_file = &globals.get_env_path(env_name).join(".viva_env.json");
        //
        // let parent = env_spec_file.parent().unwrap();
        //
        // std::fs::create_dir_all(&parent).unwrap();
        //
        // viva_env_orig.viva_env.write_env_spec();
        //
        // let specs: Option<Vec<String>> = None;
        // let channels: Option<Vec<String>> = None;
        //
        // let viva_env = VivaEnvStatus::init_env(viva_env_orig.viva_env.target_prefix, specs, channels, EnvLoadStrategy::Force, &globals).await.unwrap();
        // assert_eq!(viva_env.env_name, "test_env");
        // assert_eq!(viva_env.channels, vec!["conda-forge"]); // this should be channel_1, channel_2, but since we are not creating the env, it is not getting the channels from the spec file
        // assert_eq!(viva_env.target_prefix, parent);
        // assert_eq!(viva_env.specs, Vec::<String>::new()); // this should be channel_1, channel_2, but since we are not creating the env, it is not getting the actual specs from the spec file
        //
        // // Clean up
        // std::fs::remove_file(env_spec_file).unwrap();
    }
}
