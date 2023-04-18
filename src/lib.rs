mod defaults;
mod environment;
mod errors;
mod rattler;
mod status;

use std::borrow::Cow;
pub use crate::rattler::global_multi_progress;
pub use crate::rattler::writer::IndicatifWriter;
pub use defaults::DEFAULT_CHANNELS;
use directories::ProjectDirs;
pub use environment::EnvLoadStrategy;
pub use environment::{VivaEnv, VivaEnvStatus};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::join;
#[macro_use] extern crate prettytable;
use prettytable::{Table, Row, Cell, format};


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
        let path = self
            .project_dirs
            .config_dir()
            .join("envs")
            .join(format!("{}.json", env_name));
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
                        let viva_env = VivaEnv::load(env_name, &self)
                            .await
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

    pub async fn pretty_print_envs(&self) {

        let envs = self.list_envs().await;
        let mut env_names: Vec<String> = envs.keys().map(|k| k.to_string()).collect();
        env_names.sort();

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_titles(row!["name", "path", "specs", "channels"]);
        for env in env_names {
            let viva_env = envs.get(&env).unwrap();
            let path = viva_env.target_prefix.to_str().unwrap();
            let specs = viva_env.specs.join("\n");
            let channels = viva_env.channels.join("\n");
            table.add_row(row![env, path, specs, channels]);
        }
        table.printstd();
    }
}
