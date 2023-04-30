use std::collections::BTreeMap;
use std::fmt::Debug;
use crate::context::VivaContext;
use crate::{VivaEnv, VivaEnvSpec};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{PathBuf, MAIN_SEPARATOR};
use tokio::fs::File;
use tokio::fs;
use tokio::io::AsyncReadExt;
use serde_json::Result as SerdeJsonResult;
use serde_yaml::Result as SerdeYamlResult;
use anyhow::{anyhow, Context, Error, Result};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VivaApp {
    pub cmd: Vec<String>,
    pub env: String,
    pub env_spec: VivaEnvSpec,
}

impl VivaApp {
    pub(crate) async fn read_app_spec(app_spec_file: &PathBuf) -> Result<VivaApp> {
        match app_spec_file.exists() {
            true => {
                if app_spec_file.is_dir() {
                    return Err(anyhow!(
                        "App specification file is a directory: {}",
                        app_spec_file.display()
                    ));
                }
            }
            false => {
                return Err(anyhow!(
                    "App specification file does not exist: {}",
                    app_spec_file.display()
                ))
            }
        };

        let mut file = File::open(app_spec_file).await?;
        let mut app_spec_data = String::new();
        file.read_to_string(&mut app_spec_data).await?;

        match VivaApp::parse_app_spec(&app_spec_data).await {
            Ok(app_spec) => {
                return Ok(app_spec);
            }
            Err(_) => {
                return Err(anyhow!(
                    "Unable to parse environment specification file: {}",
                    app_spec_file.display()
                ));
            }
        }
    }

    pub(crate) async fn parse_app_spec(app_spec_data: &str) -> Result<VivaApp> {
        let json_result: SerdeJsonResult<VivaApp> = serde_json::from_str(&app_spec_data);
        // TODO: check that alias is valid
        match json_result {
            Ok(app_spec) => {
                return Ok(app_spec);
            }
            Err(_) => {
                let yaml_result: SerdeYamlResult<VivaApp> =
                    serde_yaml::from_str(&app_spec_data);
                return yaml_result.with_context(|| {
                    format!(
                        "Unable to parse app specification string: {}",
                        app_spec_data
                    )
                });
            }
        }
    }
}

pub trait AppCollection: Debug {
    // fn init(context: &VivaContext) -> Self;
    fn get_app_names(&self) -> Vec<String>;
    fn get_app(&self, app_name: &str) -> Result<&VivaApp>;
    fn delete_app(&mut self, app_name: &str) -> Option<VivaApp>;
    fn add_app(&mut self, app_name: &str, env: VivaApp);
}


#[derive(Debug)]
pub struct DefaultAppCollection {
    base_app_config_path: PathBuf,
    registered_apps: Option<BTreeMap<String, VivaApp>>,
}

impl DefaultAppCollection {
    pub async fn create(context: &VivaContext) -> Result<Self> {
        let final_app_config_path = context.project_dirs.config_dir().join("apps");

        let mut env = DefaultAppCollection {
            base_app_config_path: final_app_config_path,
            registered_apps: None,
        };

        env.get_all_apps(false).await?;
        Ok(env)
    }

    async fn get_all_apps(&mut self, force_update: bool) -> Result<&BTreeMap<String, VivaApp>> {
        match self.registered_apps.is_none() || force_update {
            true => {
                let mut apps: BTreeMap<String, VivaApp> = BTreeMap::new();

                match self.base_app_config_path.exists() {
                    true => {
                        let JSON_EXT = std::ffi::OsStr::new("json");
                        for entry in std::fs::read_dir(&self.base_app_config_path).unwrap() {
                            let entry = entry.unwrap();
                            let spec_config_file: &PathBuf = &entry.path();
                            if spec_config_file.is_file() && Some(spec_config_file.extension()) == Some(Some(&JSON_EXT)) {
                                let app_spec: VivaApp =
                                    VivaApp::read_app_spec(&spec_config_file).await?;
                                // TODO: check if to_string_lossy is good enough here
                                let app_name: String =
                                    spec_config_file.file_stem().unwrap().to_string_lossy().into();

                                apps.insert(app_name, app_spec);
                            }
                        }
                    }
                    false => {}
                };

                self.registered_apps = Some(apps);
            }
            false => {}
        };

        Ok(self.registered_apps.as_ref().unwrap())
    }
}

impl AppCollection for DefaultAppCollection {
    fn get_app_names(&self) -> Vec<String> {
        self.registered_apps
            .as_ref()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    fn get_app(&self, app_name: &str) -> Result<&VivaApp> {
        let env = self
            .registered_apps
            .as_ref()
            .expect("No apps registered")
            .get(app_name)
            .ok_or(anyhow!("No app found with name: {}", app_name));
        env
    }

    fn delete_app(&mut self, app_name: &str) -> Option<VivaApp> {
        todo!()
        // self.registered_envs.as_mut().unwrap().remove(env_name)
    }

    fn add_app(&mut self, app_name: &str, app: VivaApp) {
        todo!()
    }

}
