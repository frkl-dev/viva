
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;
use std::path::{PathBuf};
use std::process::Stdio;


use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

use rattler_repodata_gateway::fetch::CacheAction;


use serde::{Deserialize, Serialize};
use tokio::fs;


use tokio::process::Command;
use tracing::debug;


use crate::defaults::{CONDA_BIN_DIRNAME};
use crate::models::{read_model_spec, read_models_spec, write_model_spec, write_models_spec};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum EnvSyncStatus {
    Synced,
    NotSynced,
    Unknown,
}

impl ToString for EnvSyncStatus {
    fn to_string(&self) -> String {
        match self {
            EnvSyncStatus::Synced => "Synced".to_string(),
            EnvSyncStatus::NotSynced => "Not Synced".to_string(),
            EnvSyncStatus::Unknown => "Unknown".to_string(),
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
#[allow(unused)]
fn join_pkg_specs(spec_1: &Vec<String>, spec_2: &Vec<String>) -> Vec<String> {
    let mut specs: HashSet<String> = HashSet::new();
    specs.extend(spec_1.iter().cloned());
    specs.extend(spec_2.iter().cloned());
    return specs.into_iter().collect();
}

/// Join two channel lists into a single one.
#[allow(unused)]
fn join_channels(channel_1: &Vec<String>, channel_2: &Vec<String>) -> Vec<String> {
    let mut specs: HashSet<String> = HashSet::new();
    specs.extend(channel_1.iter().cloned());
    specs.extend(channel_2.iter().cloned());
    return specs.into_iter().collect();
}

#[allow(unused)]
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

#[allow(unused)]
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
    pub collection_id: String,
    env_path: PathBuf,
    pub spec: VivaEnvSpec,
    actual_spec_path: PathBuf,
    actual: VivaEnvSpec,
    pub sync_status: EnvSyncStatus,
}

impl VivaEnvSpec {
    pub fn new() -> VivaEnvSpec {
        VivaEnvSpec {
            channels: vec![],
            pkg_specs: vec![],
        }
    }
}

impl VivaEnv {
    pub fn get_env_path(&self) -> &PathBuf {
        &self.env_path
    }

    pub fn create(
        id: String,
        collection_id: String,
        spec: VivaEnvSpec,
        env_path: PathBuf,
        actual: VivaEnvSpec,
        actual_spec_path: PathBuf,
        sync_status: EnvSyncStatus,
    ) -> VivaEnv {
        VivaEnv {
            id: id,
            collection_id,
            spec: spec,
            env_path: env_path,
            actual: actual,
            actual_spec_path: actual_spec_path,
            sync_status: sync_status,
        }
    }

    pub async fn update_spec(&mut self) -> Result<()> {
        todo!("Update spec")
        // match self.spec_path {
        //     Some(ref spec_path) => {
        //         if self.spec_changed {
        //             match spec_path.extension() {
        //                 Some(ext) => {
        //                     if ext == "json" {
        //                         let env_spec_json = serde_json::to_string(&self.spec).expect(&format!(
        //                             "Cannot serialize environment spec to JSON: {}",
        //                             &spec_path.to_string_lossy()
        //                         ));
        //                         let mut file = File::create(spec_path).await?;
        //                         file.write_all(env_spec_json.as_bytes()).await?;
        //                     } else if ext == "yaml" || ext == "yml" {
        //                         let env_spec_yaml = serde_yaml::to_string(&self.spec).expect(&format!(
        //                             "Cannot serialize environment spec to YAML: {}",
        //                             &spec_path.to_string_lossy()
        //                         ));
        //                         let mut file = File::create(spec_path).await?;
        //                         file.write_all(env_spec_yaml.as_bytes()).await?;
        //                     } else {
        //                         return Err(anyhow!(
        //                             "Unable to serialize environment specification, unknown extension: {}",
        //                             ext.to_string_lossy()
        //                         ));
        //                     }
        //                 }
        //                 None => {
        //                     let env_spec_yaml = serde_yaml::to_string(&self.spec).expect(&format!(
        //                         "Cannot serialize environment spec to YAML: {}",
        //                         &spec_path.to_string_lossy()
        //                     ));
        //                     let mut file = File::create(spec_path).await?;
        //                     file.write_all(env_spec_yaml.as_bytes()).await?;
        //                 }
        //             }
        //         }
        //
        //         self.spec_changed = false;
        //         Ok(())
        //
        //     }
        //     // fine, in this case we don't need to update the spec file
        //     None => {
        //         Ok(())
        //     }
        // }
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
    pub async fn sync(&mut self) -> Result<bool> {
        if self.sync_status == EnvSyncStatus::Unknown {
            debug!("Calculating sync status for environment: {:?}", &self.id);
            self.check_and_update_sync_status();
        }

        if self.sync_status == EnvSyncStatus::Synced {
            debug!(
                "Environment does not need to be updated, status is synced: {:?}",
                &self
            );
            return Ok(false);
        }

        debug!("Updating environment: {:?}", &self);

        let cache_action = CacheAction::CacheOrFetch;
        let create_result =
            crate::rattler::commands::create::create(&self.env_path, &self.spec, cache_action)
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

                if let Some(parent_dir) = env_spec_file.parent() {
                    tokio::fs::create_dir_all(parent_dir).await?;
                }

                tokio::fs::write(&env_spec_file, env_spec_json)
                    .await
                    .expect(&format!(
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

    pub fn check_and_update_sync_status(&mut self) {
        let sync_status = match self.spec.is_satisfied_by(&self.actual) {
            true => EnvSyncStatus::Synced,
            false => EnvSyncStatus::NotSynced,
        };
        self.sync_status = sync_status;
    }

    pub fn merge_spec(&mut self, spec: &VivaEnvSpec) -> Result<()> {
        self.add_channels(&spec.channels)
            .expect("Failed to merge channels");
        self.add_pkg_specs(&spec.pkg_specs)
            .expect("Failed to merge package specs");
        Ok(())
    }

    pub fn add_channels(&mut self, channels: &Vec<String>) -> Result<&Vec<String>> {
        for channel in channels {
            if !self.spec.channels.contains(channel) {
                self.spec.channels.push(channel.clone());
                self.sync_status = EnvSyncStatus::Unknown;
            }
        }
        self.check_and_update_sync_status();
        Ok(&self.spec.channels)
    }

    pub fn remove_channels(&mut self, channels: Vec<String>) -> Result<&Vec<String>> {
        self.spec.channels.retain(|c| !channels.contains(c));
        Ok(&self.spec.channels)
    }

    pub fn add_pkg_specs(&mut self, pkg_specs: &Vec<String>) -> Result<&Vec<String>> {
        for pkg_spec in pkg_specs {
            if !self.spec.pkg_specs.contains(pkg_spec) {
                self.spec.pkg_specs.push(pkg_spec.clone());
                self.sync_status = EnvSyncStatus::Unknown;
            }
        }
        self.check_and_update_sync_status();
        Ok(&self.spec.pkg_specs)
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
    async fn get_env_ids(&self) -> Vec<String>;
    async fn get_env(&self, env_id: &str) -> Result<&VivaEnvSpec>;
    async fn delete_env(&mut self, env_id: &str) -> Result<()>;
    async fn set_env(&mut self, env_id: &str, env: &VivaEnvSpec) -> Result<()>;
}

#[derive(Debug)]
pub struct DefaultEnvCollection {
    // base_env_path: PathBuf,
    base_config_path: PathBuf,

    collected_envs: Option<BTreeMap<String, VivaEnvSpec>>,
    single_envs: Option<BTreeMap<String, VivaEnvSpec>>,

    collected_envs_dirty: bool,
    single_envs_dirty: Vec<String>
}

impl DefaultEnvCollection {
    pub async fn create(base_config_path: PathBuf) -> Result<Self> {
        let mut env = DefaultEnvCollection {
            base_config_path,
            collected_envs: None,
            single_envs: None,
            collected_envs_dirty: false,
            single_envs_dirty: Vec::new()
        };

        env.load_registered_envs(false).await?;
        Ok(env)
    }

    fn find_collected_envs_file(&self) -> PathBuf {

        let mut envs_file = self.base_config_path.join("envs.json");
        if !envs_file.exists() {
            envs_file.set_extension("yaml");
        }
        envs_file

    }

    fn find_single_env_file(&self, env_id: &str) -> PathBuf {

        let mut env_file = self.base_config_path.join("envs").join(env_id);
        env_file.set_extension("json");
        if !env_file.exists() {
            env_file.set_extension("yaml");
        }
        env_file

    }

    async fn load_registered_envs(&mut self, force_update: bool) -> Result<()> {

        let mut single_envs: BTreeMap<String, VivaEnvSpec> = BTreeMap::new();
        let mut collected_envs: BTreeMap<String, VivaEnvSpec>;

        let mut collected_envs_dirty: bool = false;

        match self.collected_envs.is_none() || force_update {
            true => match self.base_config_path.exists() {
                true => {
                    let envs_file = self.find_collected_envs_file();

                    if envs_file.exists() {
                        collected_envs = read_models_spec(&envs_file).await?;
                    } else {
                        collected_envs = BTreeMap::new();
                    }

                    let envs_subdir = &self.base_config_path.join("envs");
                    if envs_subdir.is_dir() {
                        for entry in std::fs::read_dir(envs_subdir).unwrap() {
                            let entry = entry.unwrap();
                            let spec_config_file: &PathBuf = &entry.path();

                            if spec_config_file.is_file() {
                                let env_spec: VivaEnvSpec =
                                    read_model_spec(&spec_config_file).await?;
                                let env_id: String = spec_config_file
                                    .file_stem()
                                    .unwrap()
                                    .to_string_lossy()
                                    .into();

                                if collected_envs.contains_key(&env_id) {
                                    debug!(
                                        "Overwriting env {}, as it has it's own spec file.",
                                        env_id
                                    );
                                    collected_envs.remove(&env_id);
                                    collected_envs_dirty = true;
                                }
                                single_envs.insert(env_id, env_spec);
                            }
                        }
                    }
                }
                false => {
                    collected_envs = BTreeMap::new();
                }
            },
            false => {
                collected_envs = BTreeMap::new();
            }
        }
        self.single_envs = Some(single_envs);
        self.collected_envs = Some(collected_envs);
        self.collected_envs_dirty = collected_envs_dirty;
        Ok(())
    }

    async fn sync_config(&mut self) -> Result<()> {

        // TODO: handle changed envs in collected_envs
        if self.collected_envs_dirty {
            let envs_file = self.find_collected_envs_file();
            match &self.collected_envs {
                Some(map) => {
                    write_models_spec(&envs_file, map).await?;
                },
                None => {
                    if envs_file.exists() {
                        fs::remove_file(&envs_file).await?;
                    }
                }
            };
        }

        for env_id in &self.single_envs_dirty {
            let env_file = self.find_single_env_file(&env_id);

            match self.single_envs.as_ref().unwrap().get(env_id) {
                Some(env_spec) => {
                    write_model_spec(&env_file, env_spec).await?;
                },
                None => {
                    if env_file.exists() {
                        fs::remove_file(&env_file).await?;
                    }
                }
            }
        }

        Ok(())

    }
}

#[async_trait]
impl EnvironmentCollection for DefaultEnvCollection {

    async fn get_env_ids(&self) -> Vec<String> {
        let mut collected: Vec<String> = self.collected_envs
            .as_ref()
            .unwrap()
            .iter()
            .map(|(k, _)| String::from(k))
            .collect();
        let mut single: Vec<String> = self.single_envs
            .as_ref()
            .unwrap()
            .iter()
            .map(|(k, _)| String::from(k))
            .collect();

        collected.append(&mut single);
        collected

    }

    async fn get_env(&self, env_id: &str) -> Result<&VivaEnvSpec> {

        let mut envs = self.single_envs.as_ref().unwrap();

        if ! envs.contains_key(env_id) {
            envs =  self.collected_envs.as_ref().unwrap();
        }
        let env = envs
            .get(env_id)
            .ok_or(anyhow!("No env found with name: {}", env_id));
        env
    }

    async fn delete_env(&mut self, env_id: &str) -> Result<()> {

        match self.single_envs.as_ref() {
            Some(envs) => {
                if envs.contains_key(env_id) {
                    self.single_envs_dirty.push(env_id.to_string());
                    self.single_envs.as_mut().unwrap().remove(env_id);
                }
            },
            None => {}
        }

        self.sync_config().await?;

        Ok(())
    }

    async fn set_env(&mut self, env_id: &str, env_spec: &VivaEnvSpec) -> Result<()> {
        // if self.get_env_ids().await.iter().any(|s| s == env_id) {
        //     return Err(anyhow!("Environment with id '{}' already exists", env_id));
        // }

        let spec_config_file = self
            .base_config_path
            .join("envs")
            .join(format!("{}.yaml", env_id));
        // TODO: check if already exists

        write_model_spec(&spec_config_file, env_spec).await?;
        self.single_envs
            .as_mut()
            .unwrap()
            .insert(env_id.to_string(), env_spec.clone());

        self.sync_config().await?;
        Ok(())
    }


}

#[cfg(test)]
mod tests {
    
    

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
