use crate::{VivaEnv, VivaEnvSpec};
use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

#[macro_use]
use crate::defaults::{ENV_SPEC_FILENAME};
use crate::environment::EnvironmentCollection;
use prettytable::{format, Table};
use regex::Regex;
use tracing::debug;

/// a struct that holds the global app configuration
#[derive(Debug)]
pub struct VivaContext {
    pub qualifier: String,
    pub organization: String,
    pub application: String,
    pub project_dirs: ProjectDirs,
    env_collections: HashMap<String, Box<dyn EnvironmentCollection>>,
}

impl VivaContext {
    /// create a new Globals struct for the viva library
    pub fn init() -> VivaContext {
        VivaContext::create("dev", "frkl", "viva", None, None, None)
    }

    /// create a new Globals struct for a 3rd party application
    #[allow(dead_code)]
    pub fn create(
        qualifier: &str,
        organization: &str,
        application: &str,
        base_env_path: Option<&PathBuf>,
        base_env_config_path: Option<PathBuf>,
        base_app_config_path: Option<&PathBuf>,
    ) -> VivaContext {
        let project_dirs = ProjectDirs::from(qualifier, organization, application)
            .expect("Cannot create project directories");

        let final_env_path = match base_env_path {
            Some(p) => p.clone(),
            None => project_dirs.data_dir().join("envs"),
        };

        let final_env_config_path = match base_env_config_path {
            Some(p) => p.clone(),
            None => project_dirs.config_dir().join("envs"),
        };
        let final_app_config_path = match base_app_config_path {
            Some(p) => p.clone(),
            None => project_dirs.config_dir().join("apps"),
        };

        VivaContext {
            qualifier: String::from(qualifier),
            organization: String::from(organization),
            application: String::from(application),
            project_dirs: project_dirs,
            env_collections: HashMap::new(),
        }
    }

    pub fn add_env_collection(&mut self, col_name: &str, collection: Box<dyn EnvironmentCollection>) {
        self.env_collections.insert(String::from(col_name), collection);
    }

    pub async fn list_envs(&self) -> BTreeMap<String, &VivaEnv> {
        let mut envs: BTreeMap<String, &VivaEnv> = BTreeMap::new();
        for (col_name, col) in &self.env_collections {
            for env_name in col.get_env_names().await {
                match envs.contains_key(&env_name) {
                    true => {
                        debug!("Skipping duplicate environment: {}", &env_name);
                    }
                    false => {
                        envs.insert(
                            env_name.clone(),
                            col.get_env(&env_name).await.expect(
                                format!("Can't lookup environment: {}", &env_name).as_str(),
                            ),
                        );
                    }
                }
            }
        }
        envs
    }

    pub async fn get_env_names(&self) -> Vec<String> {

        let mut envs: Vec<String> = Vec::new();
        for (col_name, col) in &self.env_collections {
            for env_name in col.get_env_names().await {
                match envs.contains(&env_name) {
                    true => {
                        debug!("Skipping duplicate environment: {}", &env_name);
                    }
                    false => {
                        envs.push(env_name.clone());
                    }
                }
            }
        }
        envs.sort();
        envs
    }

    pub async fn has_env(&self, env_name: &str) -> bool {
        self.get_env_names().await.iter().any(|s| s.as_str() == env_name)
    }

    pub async fn add_env(&mut self, env_name: &str, env: Option<VivaEnvSpec>, env_collection: Option<&str>) -> Result<&VivaEnv> {
        let env_col_name = match env_collection {
            Some(col_name) => col_name,
            None => "default",
        };

        let env_col = self.env_collections.get_mut(env_col_name).expect(
            format!("Environment collection not found: {}", env_col_name).as_str(),
        ).as_mut();

        match env {
            Some(env) => {
                env_col.set_env(env_name, &env).await
            }
            None => {
                env_col.set_env(env_name, &VivaEnvSpec::new()).await
            }
        }

    }

    pub async fn get_env(&self, env_name: &str) -> Result<&VivaEnv> {
        let comp_env_name = &env_name.to_string();
        for (col_name, col) in &self.env_collections {
            if col.get_env_names().await.contains(comp_env_name) {
                return Ok(col.get_env(env_name).await?);
            }
        }
        Err(anyhow!("Environment not found: {}", env_name))
    }

    pub async fn get_env_mut(&mut self, env_name: &str) -> Result<&mut VivaEnv> {
        let comp_env_name = &env_name.to_string();
        for (col_name, col) in self.env_collections.iter_mut() {
            if col.get_env_names().await.contains(comp_env_name) {
                return Ok(col.get_env_mut(env_name).await?);
            }
        }
        Err(anyhow!("Environment not found: {}", env_name))
    }

    pub async fn pretty_print_envs(&self) {
        let envs = self.list_envs().await;
        let mut env_names: Vec<String> = envs.keys().map(|k| k.to_string()).collect();
        env_names.sort();

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_titles(prettytable::row!["name", "path", "specs", "channels", "status"]);
        for env in env_names {
            let viva_env = envs.get(&env).unwrap();
            let path = viva_env.env_path.to_str().unwrap();
            let specs = viva_env.spec.pkg_specs.join("\n");
            let channels = viva_env.spec.channels.join("\n");
            let status = &viva_env.sync_status;
            table.add_row(prettytable::row![env, path, specs, channels, status]);
        }
        table.printstd();
    }

    // pub async fn pretty_print_apps(&mut self) {
    //     let envs = self.list_apps().await;
    //     let mut app_names: Vec<String> = envs.keys().map(|k| k.to_string()).collect();
    //     app_names.sort();
    //
    //     let mut table = Table::new();
    //     table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    //     table.set_titles(row!["name", "command", "env"]);
    //     for app in app_names {
    //         let viva_app = envs.get(&app).unwrap();
    //         let cmd = viva_app.cmd.join(" ");
    //         let env_path = viva_app.viva_env.target_prefix.to_str().unwrap();
    //         table.add_row(row![app, cmd, env_path]);
    //     }
    //     table.printstd();
    // }
}
