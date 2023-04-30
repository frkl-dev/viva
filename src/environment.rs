use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::path::{PathBuf, MAIN_SEPARATOR};
use std::process::Stdio;
use std::str::FromStr;

use anyhow::{anyhow, Context, Error, Result};
use directories::ProjectDirs;
use rattler_repodata_gateway::fetch::CacheAction;
use regex::Regex;
use serde::de::Expected;
use serde::{Deserialize, Serialize};
use serde_json::Result as SerdeJsonResult;
use serde_yaml::Result as SerdeYamlResult;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;
use crate::app::VivaApp;
use async_trait::async_trait;

use crate::context::VivaContext;
use crate::defaults::{CONDA_BIN_DIRNAME, ENV_SPEC_FILENAME};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum EnvSyncStatus {
    Synced,
    NotSynced,
}

impl ToString for EnvSyncStatus {
    fn to_string(&self) -> String {
        match self {
            EnvSyncStatus::Synced => "Synced".to_string(),
            EnvSyncStatus::NotSynced => "Not Synced".to_string(),
        }
    }
}

/// Represents the Viva environment specification.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VivaEnvSpec {
    pub channels: Vec<String>,
    pub pkg_specs: Vec<String>,
}

impl PartialEq for VivaEnvSpec {
    fn eq(&self, other: &Self) -> bool {
        if self.pkg_specs != other.pkg_specs {
            return false;
        }

        let mut sorted_channels = self.channels.clone();
        let mut sorted_channels_other = other.channels.clone();

        sorted_channels.sort();
        sorted_channels_other.sort();

        sorted_channels == sorted_channels_other
    }
}

impl Eq for VivaEnvSpec {}

/// Join two pacakge spec lists into a single one.
fn join_pkg_specs(spec_1: &Vec<String>, spec_2: &Vec<String>) -> Vec<String> {
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

fn pkg_specs_are_equal(spec_1: &Vec<String>, spec_2: &Vec<String>) -> bool {
    let mut specs_1: HashSet<String> = HashSet::new();
    specs_1.extend(spec_1.iter().cloned());
    let mut specs_2: HashSet<String> = HashSet::new();
    specs_2.extend(spec_2.iter().cloned());
    return specs_1 == specs_2;
}

fn check_for_new_pkg_specs(
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

impl VivaEnvSpec {
    pub fn is_satisfied_by(&self, other_spec: &VivaEnvSpec) -> bool {
        let new_channels = check_for_new_channels(&other_spec.channels, &self.channels);
        if !new_channels.is_empty() {
            return false;
        }

        let new_matchspecs = check_for_new_pkg_specs(&other_spec.pkg_specs, &&self.pkg_specs);
        if !new_matchspecs.is_empty() {
            return false;
        }
        return true;
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VivaEnv {
    pub id: String,
    pub env_path: PathBuf,
    spec_path: Option<PathBuf>,
    pub spec: VivaEnvSpec,
    actual_spec_path: PathBuf,
    actual: VivaEnvSpec,
    pub sync_status: EnvSyncStatus,
    spec_changed: bool
}

impl VivaEnvSpec {

    pub fn new() -> VivaEnvSpec {
        VivaEnvSpec {
            channels: vec!(),
            pkg_specs: vec!()
        }
    }

    /// Read env spec data from a file that contains multiple environments.
    pub(crate) async fn read_envs_spec(env_specs_file: &PathBuf) -> Result<BTreeMap<String, VivaEnvSpec>> {
        match env_specs_file.exists() {
            true => {
                if env_specs_file.is_dir() {
                    return Err(anyhow!(
                        "Environments specification file is a directory: {}",
                        env_specs_file.display()
                    ));
                }
            }
            false => {
                return Err(anyhow!(
                    "Environments specification file does not exist: {}",
                    env_specs_file.display()
                ))
            }
        };

        let mut file = File::open(env_specs_file).await?;
        let mut env_specs_data = String::new();
        file.read_to_string(&mut env_specs_data).await?;

        match VivaEnvSpec::parse_envs_spec(&env_specs_data) {
            Ok(envs_spec) => {
                return Ok(envs_spec);
            }
            Err(_) => {
                return Err(anyhow!(
                    "Unable to parse environment specification file: {}",
                    env_specs_file.display()
                ));
            }
        }
    }

     pub(crate) fn parse_envs_spec_json(env_spec_data: &str) -> Result<BTreeMap<String, VivaEnvSpec>> {
        let json_result: SerdeJsonResult<BTreeMap<String, VivaEnvSpec>> = serde_json::from_str(&env_spec_data);
        match json_result {
            Ok(env_spec) => {
                return Ok(env_spec);
            }
            Err(_) => {
                return Err(anyhow!(
                    "Unable to parse environment specification json: {}",
                    env_spec_data
                ));
            }
        }
    }

    pub(crate) fn parse_envs_spec_yaml(env_spec_data: &str) -> Result<BTreeMap<String, VivaEnvSpec>> {
        let json_result = serde_yaml::from_str(&env_spec_data);
        match json_result {
            Ok(env_spec) => {
                return Ok(env_spec);
            }
            Err(_) => {
                return Err(anyhow!(
                    "Unable to parse environment specification json: {}",
                    env_spec_data
                ));
            }
        }
    }

    pub(crate) fn parse_envs_spec(env_spec_data: &str) -> Result<BTreeMap<String, VivaEnvSpec>> {

        let json_result = VivaEnvSpec::parse_envs_spec_json(env_spec_data);

        // TODO: check that alias is valid
        match json_result {
            Ok(env_spec) => {
                return Ok(env_spec);
            }
            Err(_) => {
                let yaml_result = VivaEnvSpec::parse_envs_spec_yaml(env_spec_data);
                return yaml_result.with_context(|| {
                    format!(
                        "Unable to parse environment specification yaml: {}",
                        env_spec_data
                    )
                });
            }
        }
    }

    /// Read env spec data from a file.
    pub(crate) async fn read_env_spec(env_spec_file: &PathBuf) -> Result<VivaEnvSpec> {
        match env_spec_file.exists() {
            true => {
                if env_spec_file.is_dir() {
                    return Err(anyhow!(
                        "Environment specification file is a directory: {}",
                        env_spec_file.display()
                    ));
                }
            }
            false => {
                return Err(anyhow!(
                    "Environment specification file does not exist: {}",
                    env_spec_file.display()
                ))
            }
        };

        let mut file = File::open(env_spec_file).await?;
        let mut env_spec_data = String::new();
        file.read_to_string(&mut env_spec_data).await?;

        match env_spec_file.extension() {
            Some(ext) => {
                if ext == "json" {
                    return VivaEnvSpec::parse_env_spec_json(&env_spec_data);
                } else if ext == "yaml" || ext == "yml" {
                    return VivaEnvSpec::parse_env_spec_yaml(&env_spec_data);
                } else {
                return Err(anyhow!(
                    "Unable to parse environment specification file, unknown extension: {}",
                    ext.to_string_lossy()
                ));

                }

            }
            None => {
                return VivaEnvSpec::parse_env_spec(&env_spec_data);
            }
        }
    }

    pub(crate) fn parse_env_spec_json(env_spec_data: &str) -> Result<VivaEnvSpec> {
        let json_result: SerdeJsonResult<VivaEnvSpec> = serde_json::from_str(&env_spec_data);
        match json_result {
            Ok(env_spec) => {
                return Ok(env_spec);
            }
            Err(_) => {
                return Err(anyhow!(
                    "Unable to parse environment specification json: {}",
                    env_spec_data
                ));
            }
        }
    }

    pub(crate) fn parse_env_spec_yaml(env_spec_data: &str) -> Result<VivaEnvSpec> {
        let json_result = serde_yaml::from_str(&env_spec_data);
        match json_result {
            Ok(env_spec) => {
                return Ok(env_spec);
            }
            Err(_) => {
                return Err(anyhow!(
                    "Unable to parse environment specification json: {}",
                    env_spec_data
                ));
            }
        }
    }

    pub(crate) fn parse_env_spec(env_spec_data: &str) -> Result<VivaEnvSpec> {

        let json_result = VivaEnvSpec::parse_env_spec_json(env_spec_data);

        // TODO: check that alias is valid
        match json_result {
            Ok(env_spec) => {
                return Ok(env_spec);
            }
            Err(_) => {
                let yaml_result = VivaEnvSpec::parse_env_spec_yaml(env_spec_data);
                return yaml_result.with_context(|| {
                    format!(
                        "Unable to parse environment specification yaml: {}",
                        env_spec_data
                    )
                });
            }
        }
    }
}

impl VivaEnv {

    // pub fn new(id: &str, env_path: &PathBuf) -> VivaEnv {
    //     let actual_spec_path = env_path.join(ENV_SPEC_FILENAME);
    //     VivaEnv {
    //         id: String::from(id),
    //         spec: VivaEnvSpec::new(),
    //         spec_path: None,
    //         env_path: env_path.clone(),
    //         actual: VivaEnvSpec::new(),
    //         actual_spec_path,
    //         sync_status: EnvSyncStatus::Synced,
    //         spec_changed: false
    //     }
    // }

    pub async fn update_spec_file(&mut self) -> Result<()> {
        match self.spec_path {
            Some(ref spec_path) => {
                if self.spec_changed {
                    match spec_path.extension() {
                        Some(ext) => {
                            if ext == "json" {
                                let env_spec_json = serde_json::to_string(&self.spec).expect(&format!(
                                    "Cannot serialize environment spec to JSON: {}",
                                    &spec_path.to_string_lossy()
                                ));
                                let mut file = File::create(spec_path).await?;
                                file.write_all(env_spec_json.as_bytes()).await?;
                            } else if ext == "yaml" || ext == "yml" {
                                let env_spec_yaml = serde_yaml::to_string(&self.spec).expect(&format!(
                                    "Cannot serialize environment spec to YAML: {}",
                                    &spec_path.to_string_lossy()
                                ));
                                let mut file = File::create(spec_path).await?;
                                file.write_all(env_spec_yaml.as_bytes()).await?;
                            } else {
                                return Err(anyhow!(
                                    "Unable to serialize environment specification, unknown extension: {}",
                                    ext.to_string_lossy()
                                ));
                            }
                        }
                        None => {
                            let env_spec_yaml = serde_yaml::to_string(&self.spec).expect(&format!(
                                "Cannot serialize environment spec to YAML: {}",
                                &spec_path.to_string_lossy()
                            ));
                            let mut file = File::create(spec_path).await?;
                            file.write_all(env_spec_yaml.as_bytes()).await?;
                        }
                    }
                }

                self.spec_changed = false;
                Ok(())

            }
            // fine, in this case we don't need to update the spec file
            None => {
                Ok(())
            }
        }

    }

    /// Applies the environments specs and ensures it is created and ready to be used.
    ///
    /// Be aware that this could remove some packages that already exist in the environment.
    ///
    /// # Arguments
    ///
    /// * `update_spec_file` - whether to update the spec file for the environment (if there is one)
    ///
    /// # Returns
    ///
    /// Returns false if the environment didn't need to be synced, true if it did, and an error if there was a problem.
    pub async fn apply(&mut self, update_spec_file: bool) -> Result<bool> {


        if update_spec_file {
            self.update_spec_file().await?;
        }

        if self.sync_status == EnvSyncStatus::Synced {
            debug!("Environment does not need to be updated, status is synced: {:?}", &self);
            return Ok(false);
        }

        debug!("Updating environment: {:?}", &self);

        let cache_action = CacheAction::CacheOrFetch;
        let create_result = crate::rattler::commands::create::create(&self.env_path, &self.spec, cache_action)
            .await
            .with_context(|| format!("Failed to create environment: {:?}", &self));

        debug!("Environment created: {:?}", &create_result);
        match create_result {
            Ok(_) => {

                // TODO: delete created env if this fails?
                let env_spec_file = &self.actual_spec_path;

                let env_spec_json = serde_json::to_string(&self.spec).expect(&format!(
                    "Cannot serialize environment spec to JSON: {}",
                    &env_spec_file.to_string_lossy()
                ));

                std::fs::write(&env_spec_file, env_spec_json).expect(&format!(
                    "Cannot write environment spec to file: {}",
                    &env_spec_file.to_string_lossy()
                ));

                self.actual = self.spec.clone();
                self.sync_status = EnvSyncStatus::Synced;

                Ok(true)
            }
            Err(e) => {
                debug!("Failed to create environment: {:?}", &e);
                Err(e)
            }
        }
    }

    fn update_sync_status(&mut self) {
        let sync_status = match self.spec.is_satisfied_by(&self.actual) {
            true => EnvSyncStatus::Synced,
            false => EnvSyncStatus::NotSynced,
        };
        self.sync_status = sync_status;

    }

    pub fn add_channels(&mut self, channels: &Vec<String>) -> Result<&Vec<String>> {
        for channel in channels {
            if !self.spec.channels.contains(channel) {
                self.spec.channels.push(channel.clone());
                self.spec_changed = true;
            }
        }
        self.update_sync_status();
        Ok(&self.spec.channels)
    }

    pub fn remove_channels(&mut self, channels: Vec<String>) -> Result<&Vec<String>> {
        self.spec.channels.retain(|c| !channels.contains(c));
        self.spec_changed = true;
        Ok(&self.spec.channels)
    }

    pub fn add_pkg_specs(&mut self, pkg_specs: &Vec<String>) -> Result<&Vec<String>> {
        for pkg_spec in pkg_specs {
            if !self.spec.pkg_specs.contains(pkg_spec) {
                self.spec.pkg_specs.push(pkg_spec.clone());
                self.spec_changed = true;
            }
        }
        self.update_sync_status();
        Ok(&self.spec.pkg_specs)
    }


    pub async fn delete(&self) -> Result<()> {
        match self.env_path.exists() {
            true => fs::remove_dir_all(&self.env_path).await?,
            false => {},
        };
        match self.spec_path {
            Some(ref spec_path) => match spec_path.exists() {
                true => match fs::remove_file(&spec_path).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!("Failed to remove environment spec: {}", e)),
                },
                false => Ok(()),
            },
            None => Ok(()),
        }
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
        let mut full_exe_path = self.env_path.join(CONDA_BIN_DIRNAME).join(executable);

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
        // unsafe { child.detach() };fffbbb

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{}", stdout);
        } else {
            eprintln!("{:?}", output);
        }

        Ok(())
    }
}

#[async_trait]
pub trait EnvironmentCollection: Debug {
    // fn init(context: &VivaContext) -> Self;
    async fn get_env_names(&self) -> Vec<String>;
    async fn get_env(&self, env_name: &str) -> Result<&VivaEnv>;
    async fn get_env_mut(&mut self, env_name: &str) -> Result<&mut VivaEnv>;
    async fn delete_env(&mut self, env_name: &str) -> Option<VivaEnv>;
    async fn set_env(&mut self, env_name: &str, env: &VivaEnvSpec) -> Result<&VivaEnv>;
}

#[derive(Debug)]
pub struct DefaultEnvCollection {
    base_env_path: PathBuf,
    base_config_path: PathBuf,

    registered_envs: Option<BTreeMap<String, VivaEnv>>,
}

impl DefaultEnvCollection {
    pub async fn create(context: &VivaContext) -> Result<Self> {
        let final_env_path = context.project_dirs.data_dir().join("envs");

        let final_config_path = PathBuf::from(context.project_dirs.config_dir());

        let mut env = DefaultEnvCollection {
            base_env_path: final_env_path,
            base_config_path: final_config_path,
            registered_envs: None,
        };

        env.get_all_envs(false).await?;
        Ok(env)
    }

    async fn get_all_envs(&mut self, force_update: bool) -> Result<&BTreeMap<String, VivaEnv>> {
        match self.registered_envs.is_none() || force_update {
            true => {
                let mut envs: BTreeMap<String, VivaEnv> = BTreeMap::new();

                match self.base_config_path.exists() {
                    true => {
                        let JSON_EXT = std::ffi::OsStr::new("json");
                        let mut envs_file = self.base_config_path.join("envs.json");
                        if !envs_file.exists() {
                            envs_file.set_extension("yaml");
                        }
                        if envs_file.exists() {
                            for (env_name, env) in VivaEnvSpec::read_envs_spec(&envs_file).await?.into_iter() {
                                let env_path: PathBuf = self.base_env_path.join(&env_name);
                                let env_spec_file: PathBuf = env_path.join("viva_env.json");
                                let actual_env_spec: VivaEnvSpec = match env_spec_file.exists() {
                                    true => {
                                        let env_actual = VivaEnvSpec::read_env_spec(&env_path).await?;
                                        env_actual
                                    }
                                    false => {
                                        VivaEnvSpec {
                                            channels: vec![],
                                            pkg_specs: vec![],
                                        }
                                    }
                                };
                                let sync_status = match env.is_satisfied_by(&actual_env_spec) {
                                    true => EnvSyncStatus::Synced,
                                    false => EnvSyncStatus::NotSynced,
                                };
                                let viva_env: VivaEnv = VivaEnv {
                                    id: env_name.clone(),
                                    spec_path: Some(envs_file.clone()),
                                    spec: env,
                                    env_path: env_path.clone(),
                                    actual: actual_env_spec,
                                    actual_spec_path: env_path.join(ENV_SPEC_FILENAME),
                                    sync_status: sync_status,
                                    spec_changed: false
                                };
                                envs.insert(env_name, viva_env);
                            }

                        };

                        for entry in std::fs::read_dir(&self.base_config_path.join("envs")).unwrap() {
                            let entry = entry.unwrap();
                            let spec_config_file: &PathBuf = &entry.path();
                            if spec_config_file.is_file() && Some(spec_config_file.extension()) == Some(Some(&JSON_EXT))  {
                                let env_spec: VivaEnvSpec =
                                    VivaEnvSpec::read_env_spec(&spec_config_file).await?;
                                // TODO: check if to_string_lossy is good enough here
                                let env_name: String =
                                    spec_config_file.file_stem().unwrap().to_string_lossy().into();

                                if envs.contains_key(&env_name) {
                                    debug!("Overwriting env {}, as it has it's own spec file.", env_name);
                                }

                                let env_path: PathBuf = self.base_env_path.join(&env_name);
                                let env_spec_file: PathBuf = env_path.join("viva_env.json");
                                let actual_env_spec: VivaEnvSpec = match env_spec_file.exists() {
                                    true => {
                                        let env_actual = VivaEnvSpec::read_env_spec(&env_spec_file).await?;
                                        env_actual
                                    }
                                    false => {
                                        VivaEnvSpec {
                                            channels: vec![],
                                            pkg_specs: vec![],
                                        }
                                    }
                                };
                                let sync_status = match env_spec.is_satisfied_by(&actual_env_spec) {
                                    true => EnvSyncStatus::Synced,
                                    false => EnvSyncStatus::NotSynced,
                                };
                                let viva_env: VivaEnv = VivaEnv {
                                    id: env_name.clone(),
                                    spec_path: Some(spec_config_file.clone()),
                                    spec: env_spec,
                                    env_path: env_path,
                                    actual: actual_env_spec,
                                    actual_spec_path: env_spec_file,
                                    sync_status: sync_status,
                                    spec_changed: false
                                };

                                envs.insert(env_name, viva_env);
                            }
                        }
                    }
                    false => {}
                };

                self.registered_envs = Some(envs);
            }
            false => {}
        };

        Ok(self.registered_envs.as_ref().unwrap())
    }
}

#[async_trait]
impl EnvironmentCollection for DefaultEnvCollection {
    async fn get_env_names(&self) -> Vec<String> {
        self.registered_envs
            .as_ref()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }


    async fn get_env(&self, env_name: &str) -> Result<&VivaEnv> {
        let env = self
            .registered_envs
            .as_ref()
            .expect("No envs registered")
            .get(env_name)
            .ok_or(anyhow!("No env found with name: {}", env_name));
        env
    }

    async fn get_env_mut(&mut self, env_name: &str) -> Result<&mut VivaEnv> {
        let mut env = self
            .registered_envs
            .as_mut()
            .expect("No envs registered")
            .get_mut(env_name);

        match env {
            Some(env) => Ok(env),
            None => Err(anyhow!("No env found with name: {}", env_name))
        }

    }

    async fn delete_env(&mut self, env_name: &str) -> Option<VivaEnv> {
        todo!()
        // self.registered_envs.as_mut().unwrap().remove(env_name)
    }

    async fn set_env(&mut self, env_name: &str, env_spec: &VivaEnvSpec) -> Result<&VivaEnv> {

        let spec_config_file = self.base_config_path.join("envs").join(format!("{}.json", env_name));
        // TODO: check if already exists
        let env_path = self.base_env_path.join(env_name);
        let actual = VivaEnvSpec::new();
        let env_spec_file: PathBuf = env_path.join("viva_env.json");

        let sync_status = match env_spec.is_satisfied_by(&actual) {
            true => EnvSyncStatus::Synced,
            false => EnvSyncStatus::NotSynced,
        };

        let mut viva_env: VivaEnv = VivaEnv {
            id: String::from(env_name),
            spec_path: Some(spec_config_file),
            spec: env_spec.clone(),
            env_path: env_path,
            actual: actual,
            actual_spec_path: env_spec_file,
            sync_status: sync_status,
            spec_changed: false
        };


        &viva_env.update_spec_file().await?;
        self.registered_envs.as_mut().unwrap().insert(env_name.to_string(), viva_env);
        self.get_env(env_name).await
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
