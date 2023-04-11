use std::borrow::Cow;
use std::collections::HashSet;
use std::hash::Hash;
use std::path::{PathBuf};
use std::process::Stdio;
use std::str::FromStr;

use anyhow::{anyhow, Result, Context, Error};
use rattler_repodata_gateway::fetch::CacheAction;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::defaults::{CONDA_BIN_DIRNAME, DEFAULT_CHANNELS, Globals};

/// Represents the strategy to use when checking whether an environment exists.
#[derive(Debug, Clone)]
pub enum EnvCheckStrategy {
    Auto,
    Skip,
    Force,
}

pub enum PkgInstallStrategy {
    Append,
    Replace,
}

/// Allows converting a string to an `EnvCheckStrategy`.
impl FromStr for EnvCheckStrategy {
    type Err = Error;

    /// Creates an `EnvCheckStrategy` from the given string.
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
            "auto" => Ok(EnvCheckStrategy::Auto),
            "skip" => Ok(EnvCheckStrategy::Skip),
            "force" => Ok(EnvCheckStrategy::Force),
            _ => Err(anyhow!("Invalid environment check strategy: {}. Available: auto, skip, force", s)),
        }
    }
}

/// Represents the Viva environment specification.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VivaEnv {

    pub env_name: String,
    pub channels: Vec<String>,
    pub target_prefix: PathBuf,
    pub specs: Vec<String>,
}

fn get_env_path(env_name: &str, globals: &Globals) -> PathBuf {
    let path = globals.project_dirs.data_dir().join("envs").join(env_name);
    return path;
}

// fn get_env_spec_path(env_name: &str, globals: &Globals) -> PathBuf {
//     let path = get_env_path(env_name, globals).join(".viva_env.json");
//     return path;
// }

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

impl VivaEnv {

    /// Constructs a `VivaEnv` from the given parameters.
    ///
    /// # Arguments
    ///
    /// * `env_name` - The name of the environment.
    /// * `specs` - An optional list of specs for the environment.
    /// * `channels` - An optional list of channels for the environment.
    /// * `globals` - A reference to a `Globals` object.
    ///
    /// # Returns
    ///
    /// Returns a `VivaEnv` if the environment configuration is successfully created, or an error if there is a problem.
    pub fn create(env_name: &str, specs: Option<Vec<String>>, channels: Option<Vec<String>>, globals: &Globals) -> anyhow::Result<Self> {

        let env_path = get_env_path(env_name, globals);
        // let env_spec_file = get_env_spec_path(env_name, globals);

        // if env_spec_file.is_file() {
        //     let env_spec = std::fs::read_to_string(&env_spec_file).expect(&format!("Cannot read environment spec from file: {}", env_spec_file.to_string_lossy()));
        //     let env_spec: VivaEnv = serde_json::from_str(&env_spec).expect(&format!("Cannot parse environment spec from file: {}", env_spec_file.to_string_lossy()));
        //
        //     // TODO: compare to provided specs/channels
        //
        //     Ok(env_spec)
        //
        // } else {
        let env_specs: Vec<String> = if let Some(specs) = specs {
            specs.into_iter().map(String::from).collect()
        } else {
            vec![]
        };
        let env_channels: Vec<String> = if let Some(chans) = channels {
            chans.into_iter().map(String::from).collect()
        } else {
            DEFAULT_CHANNELS.into_iter().map(String::from).collect()
        };
        let env_spec = VivaEnv {
            env_name: String::from(env_name),
            channels: env_channels,
            target_prefix: env_path,
            specs: env_specs,
        };

        Ok(env_spec)
        // }

    }

    fn get_env_spec_file(&self) -> PathBuf {
        let path = self.target_prefix.join(".viva_env.json");
        return path;
    }

    pub (crate) fn read_env_spec(&self) -> Option<VivaEnv> {

        let env_spec_file = &self.get_env_spec_file();
        let env_spec_json = std::fs::read_to_string(&env_spec_file).expect(&format!("Cannot read environment spec from file: {}", &env_spec_file.to_string_lossy()));
        let env_spec: VivaEnv = serde_json::from_str(&env_spec_json).expect(&format!("Cannot parse environment spec from file: {}", &env_spec_file.to_string_lossy()));
        Some(env_spec)
    }

    pub(crate) fn write_env_spec(&self) -> anyhow::Result<()> {
        let env_spec_file = &self.get_env_spec_file();
        let env_spec_json = serde_json::to_string(&self).expect(&format!("Cannot serialize environment spec to JSON: {}", &env_spec_file.to_string_lossy()));

        std::fs::write(&env_spec_file, env_spec_json).expect(&format!("Cannot write environment spec to file: {}", &env_spec_file.to_string_lossy()));
        Ok(())
    }

    pub(crate) fn merge(&self, other: &VivaEnv) -> VivaEnv {
        let env_specs = join_matchspecs(&self.specs, &other.specs);
        let env_channels = join_channels(&self.channels, &other.channels);

        let env_spec = VivaEnv {
            env_name: self.env_name.clone(),
            channels: env_channels,
            target_prefix: self.target_prefix.clone(),
            specs: env_specs,
        };
        env_spec
    }

    /// Ensures that the environment is created and ready to be used, according to the specified check strategy.
    ///
    /// # Arguments
    ///
    /// * `env_check_strategy` - The strategy to use when checking for the environment.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the environment check is successful, or an error if there is a problem.
    pub async fn ensure(&self, env_check_strategy: EnvCheckStrategy, pkg_install_strategy: PkgInstallStrategy) -> Result<()> {
        let target_prefix = PathBuf::from(&self.target_prefix);

        let skip_env_check = match env_check_strategy {
            EnvCheckStrategy::Auto => {
                if target_prefix.exists() {
                    true
                } else {
                    false
                }
            },
            EnvCheckStrategy::Skip => true,
            EnvCheckStrategy::Force => false,
        };
        if skip_env_check {
            return Ok(());
        }

        println!("Reading environment spec file: {}", self.get_env_spec_file().to_string_lossy());

        let final_env: Cow<VivaEnv> = match pkg_install_strategy {
            PkgInstallStrategy::Append => {

                let existing = match self.get_env_spec_file().exists() {
                    true => self.read_env_spec(),
                    false => None,
                };

                match &existing {
                    Some(existing) => {
                        Cow::Owned(existing.merge(self))
                    },
                    None => {
                        Cow::Borrowed(self)
                    }
                }
            },
            PkgInstallStrategy::Replace => {
                Cow::Borrowed(self)
            },
        };

        println!("Final env spec: {:?}", final_env);

        // if final_env.specs.is_empty() {
        //     return Err(anyhow!("No specs provided for environment: {}", self.env_name));
        // }

        let cache_action = CacheAction::CacheOrFetch;

        let create_result = crate::rattler::commands::create::create(&final_env, cache_action).await.with_context(|| format!("Failed to create environment: {}", &final_env.env_name));

        match create_result {
            Ok(_) => {
                // TODO: delete created env if this fails?
                self.write_env_spec().expect(&format!("Could not write environment spec file: {}", final_env.get_env_spec_file().to_string_lossy()));
                return Ok(());
            },
            Err(e) => { return Err(e) },
        }
    }

    /// Runs a command in the context of the environment, using the specified check strategy.
    ///
    /// # Arguments
    ///
    /// * `cmd` - A sequence of strings representing the command and its arguments.
    /// * `env_check_strategy` - The strategy to use when checking for the environment.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the command runs successfully, or an error if there is a problem.
    pub async fn run_command_in_env<S: AsRef<str>, I: AsRef<[S]>>(&self, cmd: I, env_check_strategy: EnvCheckStrategy) -> Result<()> {

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

        let updated_env_check_strategy: EnvCheckStrategy = match env_check_strategy {
            EnvCheckStrategy::Auto => {
                if ! full_exe_path.exists() {
                    EnvCheckStrategy::Force
                } else {
                    full_exe_path.set_extension("exe");
                    if ! full_exe_path.exists() {
                        EnvCheckStrategy::Force
                    } else {
                        EnvCheckStrategy::Auto
                    }

                }
            },
            EnvCheckStrategy::Skip => EnvCheckStrategy::Skip,
            EnvCheckStrategy::Force => EnvCheckStrategy::Force,
        };

        let pkg_install_strategy = PkgInstallStrategy::Append;

        self.ensure(updated_env_check_strategy, pkg_install_strategy).await?;

        let mut command = Command::new(full_exe_path);

        if cmd_args.len() > 0 {
            command.args(cmd_args);
        }
        let child = command
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn subprocess");

        let output = child.wait_with_output().await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("Output: {}", stdout);
        } else {
            eprintln!("Error: {:?}", output);
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::path::Path;
    use super::*;

    #[test]
    fn test_viva_env_from_str_with_spec_file() {
        let env_name = "test_env";
        let globals = Globals::create("tests", "test_org", "viva");

        let viva_env_orig = VivaEnv::create(env_name, Some(vec!(String::from("spec1"), String::from("spec2"))), Some(vec!(String::from("channel_1"), String::from("channel_2"))), &globals).unwrap();
        let env_spec_file = get_env_path(env_name, &globals).join(".viva_env.json");

        let parent = env_spec_file.parent().unwrap();

        std::fs::create_dir_all(&parent).unwrap();

        viva_env_orig.write_env_spec();

        let viva_env = VivaEnv::create(env_name, None, None, &globals).unwrap();
        assert_eq!(viva_env.env_name, "test_env");
        assert_eq!(viva_env.channels, vec!["conda-forge"]);  // this should be channel_1, channel_2, but since we are not creating the env, it is not getting the channels from the spec file
        assert_eq!(viva_env.target_prefix, parent);
        assert_eq!(viva_env.specs, Vec::<String>::new());  // this should be channel_1, channel_2, but since we are not creating the env, it is not getting the actual specs from the spec file

        // Clean up
        std::fs::remove_file(env_spec_file).unwrap();
    }
}
