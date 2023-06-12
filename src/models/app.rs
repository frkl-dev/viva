
use crate::models::environment::VivaEnvSpec;
use crate::models::{read_model_spec, read_models_spec, write_model_spec};
use anyhow::{anyhow, Result};
use async_trait::async_trait;

use serde::{Deserialize, Serialize};


use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::{PathBuf};



use tracing::debug;

pub enum AppEnvPlacementStrategy {
    Default,
    CollectionId,
    AppId,
    Custom(String),
}

impl AppEnvPlacementStrategy {
    pub fn from_str(strategy: &str) -> Result<AppEnvPlacementStrategy> {
        match strategy {
            "--default--" => Ok(AppEnvPlacementStrategy::Default),
            "--collection_id--" => Ok(AppEnvPlacementStrategy::CollectionId),
            "--app_id--" => Ok(AppEnvPlacementStrategy::AppId),
            _ => Ok(AppEnvPlacementStrategy::Custom(strategy.to_string())),
        }
    }

}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VivaAppSpec {
    pub executable: String,
    pub args: Vec<String>,
    pub env_spec: VivaEnvSpec,
}

impl PartialEq for VivaAppSpec {
    fn eq(&self, other: &Self) -> bool {
        if self.executable != other.executable {
            return false;
        }

        if self.args != other.args {
            return false;
        }

        if self.env_spec != other.env_spec {
            return false;
        }

        true
    }
}

impl Eq for VivaAppSpec {}

impl VivaAppSpec {

    pub fn get_full_cmd(&self) -> Vec<String> {
        let mut cmd = vec!(self.executable.clone());
        for arg in &self.args {
            cmd.push(arg.clone());
        }
        cmd
    }

}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VivaApp {
    pub id: String,
    pub spec: VivaAppSpec,
    pub app_collection_id: String,
    env_id: String
}

impl VivaApp {

    pub fn create(
        id: String,
        spec: VivaAppSpec,
        app_collection_id: String,
        env_id: String
    ) -> VivaApp {
        VivaApp {
            id: id,
            spec: spec,
            app_collection_id,
            env_id: env_id,
        }
    }

    pub fn get_env_id(&self) -> &str {
        &self.env_id
    }
}

#[async_trait]
pub trait AppCollection: Debug {
    async fn get_app_ids(&self) -> Vec<String>;
    async fn get_app(&self, app_id: &str) -> Result<&VivaAppSpec>;
    async fn delete_app(&mut self, app_id: &str) -> Option<VivaAppSpec>;
    async fn set_app(&mut self, app_id: &str, app_spec: &VivaAppSpec) -> Result<()>;
}

#[derive(Debug)]
pub struct DefaultAppCollection {
    base_config_path: PathBuf,
    registered_apps: Option<BTreeMap<String, VivaAppSpec>>,
}

impl DefaultAppCollection {
    pub async fn create(base_config_path: PathBuf) -> Result<Self> {
        let mut env = DefaultAppCollection {
            base_config_path,
            registered_apps: None,
        };

        env.load_registered_apps(false).await?;
        Ok(env)
    }

    async fn load_registered_apps(
        &mut self,
        force_update: bool,
    ) -> Result<&BTreeMap<String, VivaAppSpec>> {
        match self.registered_apps.is_none() || force_update {
            true => {
                let apps: BTreeMap<String, VivaAppSpec> = match self.base_config_path.exists() {
                    true => {
                        let mut app_file = self.base_config_path.join("apps.json");
                        if !app_file.exists() {
                            app_file.set_extension("yaml");
                        }

                        let mut parsed_models: BTreeMap<String, VivaAppSpec> =
                            match app_file.exists() {
                                true => read_models_spec(&app_file).await?,
                                false => BTreeMap::new(),
                            };

                        let apps_dir = self.base_config_path.join("apps");
                        if apps_dir.is_dir() {
                            for entry in std::fs::read_dir(apps_dir).unwrap() {
                                let entry = entry.unwrap();
                                let spec_config_file: &PathBuf = &entry.path();
                                if spec_config_file.is_file() {
                                    let app_spec: VivaAppSpec =
                                        read_model_spec(&spec_config_file).await?;
                                    // TODO: check if to_string_lossy is good enough here
                                    let app_name: String = spec_config_file
                                        .file_stem()
                                        .unwrap()
                                        .to_string_lossy()
                                        .into();

                                    if parsed_models.contains_key(&app_name) {
                                        debug!(
                                            "Overwriting app {}, as it has it's own spec file.",
                                            &app_name
                                        );
                                    }

                                    parsed_models.insert(app_name, app_spec);
                                }
                            }
                        }
                        parsed_models
                    }
                    false => BTreeMap::new(),
                };

                self.registered_apps = Some(apps);
            }
            false => {}
        };

        Ok(self.registered_apps.as_ref().unwrap())
    }
}

#[async_trait]
impl AppCollection for DefaultAppCollection {
    async fn get_app_ids(&self) -> Vec<String> {
        self.registered_apps
            .as_ref()
            .unwrap()
            .iter()
            .map(|(k, _)| String::from(k))
            .collect()
    }

    async fn get_app(&self, app_id: &str) -> Result<&VivaAppSpec> {
        let env = self
            .registered_apps
            .as_ref()
            .expect("No apps registered")
            .get(app_id)
            .ok_or(anyhow!("No app found with name: {}", app_id));
        env
    }

    async fn delete_app(&mut self, _app_id: &str) -> Option<VivaAppSpec> {
        todo!()
        // self.registered_envs.as_mut().unwrap().remove(env_name)
    }

    async fn set_app(&mut self, app_id: &str, app_spec: &VivaAppSpec) -> Result<()> {
        let app_spec_file = self.base_config_path.join("apps").join(format!("{}.json", app_id));
        // TOOD: check if it already exists?

        write_model_spec(&app_spec_file, app_spec).await?;
        self.registered_apps.as_mut().unwrap().insert(app_id.to_string(), app_spec.clone());

        Ok(())

    }
}
