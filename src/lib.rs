mod defaults;
mod environment;
mod errors;
mod rattler;
mod status;

pub use crate::rattler::global_multi_progress;
pub use crate::rattler::writer::IndicatifWriter;
pub use defaults::DEFAULT_CHANNELS;
use directories::ProjectDirs;
pub use environment::{VivaEnv, VivaEnvStatus};
pub use environment::{EnvLoadStrategy};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::join;

/// a struct that holds the global app configuration
#[derive(Debug)]
pub struct VivaGlobals {
    pub qualifier: String,
    pub organization: String,
    pub application: String,
    pub project_dirs: ProjectDirs,
    pub base_env_path: PathBuf,
}

impl VivaGlobals {
    #[allow(dead_code)]
    pub fn clone(&self) -> VivaGlobals {
        VivaGlobals {
            qualifier: self.qualifier.clone(),
            organization: self.organization.clone(),
            application: self.application.clone(),
            project_dirs: self.project_dirs.clone(),
            base_env_path: self.base_env_path.clone(),
        }
    }

    /// create a new Globals struct for the viva library
    pub fn new() -> VivaGlobals {
        let project_dirs =
            ProjectDirs::from("dev", "frkl", "viva").expect("Cannot create project directories");
        let base_env_path = project_dirs.data_dir().join("envs");
        VivaGlobals {
            qualifier: String::from("dev"),
            organization: String::from("frkl"),
            application: String::from("viva"),
            project_dirs: project_dirs,
            base_env_path: base_env_path,
        }
    }

    /// create a new Globals struct for a 3rd party application
    #[allow(dead_code)]
    pub fn create(
        qualifier: &str,
        organization: &str,
        application: &str,
        base_env_path: Option<&PathBuf>,
    ) -> VivaGlobals {
        let project_dirs = ProjectDirs::from(qualifier, organization, application)
            .expect("Cannot create project directories");
        let final_env_path = match base_env_path {
            Some(p) => p.clone(),
            None => project_dirs.data_dir().join("envs"),
        };

        VivaGlobals {
            qualifier: String::from(qualifier),
            organization: String::from(organization),
            application: String::from(application),
            project_dirs: project_dirs,
            base_env_path: final_env_path,
        }
    }

    // pub fn get_base_env_path(&self) -> PathBuf {
    //     let path = self.project_dirs.data_dir().join("envs");
    //     path
    // }

    pub fn get_env_data_path(&self, env_name: &str) -> PathBuf {
        let path = self.base_env_path.join(env_name);
        path
    }
    pub fn get_env_config_path(&self, env_name: &str) -> PathBuf {
        let path = self.project_dirs.config_dir().join("envs").join(format!("{}.json", env_name));
        path
    }

    pub async fn list_envs(&self) -> HashMap<String, VivaEnv> {
        println!("base_env_path: {:?}", self.base_env_path);
        let envs = match self.base_env_path.exists() {
            true => {
                let mut envs = HashMap::new();
                for entry in std::fs::read_dir(&self.base_env_path).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.is_dir() {
                        let env_name = path.file_name().unwrap().to_str().unwrap();
                        let viva_env = VivaEnv::load(env_name, &self).await
                            .expect(format!("Failed to load environment: {}", env_name).as_str());
                        envs.insert(env_name.to_string(), viva_env);
                    }
                }
                envs
            }
            false => HashMap::new(),
        };

        envs
    }
}
