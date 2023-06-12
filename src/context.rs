use anyhow::{anyhow, bail, Result};
use directories::ProjectDirs;
use std::collections::{BTreeMap, HashMap, HashSet};

use std::path::{PathBuf};

use crate::defaults::{ENV_SPEC_FILENAME};
use crate::models::app::{AppCollection, AppEnvPlacementStrategy, VivaApp, VivaAppSpec};
use crate::models::environment::{EnvSyncStatus, EnvironmentCollection, VivaEnv, VivaEnvSpec};
use crate::models::read_model_spec;
use prettytable::{format, Table};
use tokio::fs;

use tracing::debug;

/// a struct that holds the global app configuration
#[derive(Debug)]
pub struct VivaContext {
    pub project_dirs: ProjectDirs,
    env_collections: HashMap<String, Box<dyn EnvironmentCollection>>,
    app_collections: HashMap<String, Box<dyn AppCollection>>,
    registered_envs: BTreeMap<String, VivaEnv>,
    registered_apps: BTreeMap<String, VivaApp>,
    base_env_path: PathBuf,
}

impl VivaContext {
    /// create a new Globals struct for the viva library
    pub fn init() -> VivaContext {
        VivaContext::create("dev", "frkl", "viva")
    }

    pub fn create(qualifier: &str, organization: &str, application: &str) -> VivaContext {
        let project_dirs = ProjectDirs::from(qualifier, organization, application)
            .expect("Cannot create project directories");

        let base_env_path = project_dirs.data_dir().join("envs");

        VivaContext {
            project_dirs,
            env_collections: HashMap::new(),
            app_collections: HashMap::new(),
            registered_envs: BTreeMap::new(),
            registered_apps: BTreeMap::new(),
            base_env_path,
        }
    }

    pub async fn add_env_collection(
        &mut self,
        collection_id: &str,
        collection: Box<dyn EnvironmentCollection>,
    ) -> Result<()> {
        for env_id in collection.get_env_ids().await {
            let env_spec = collection.get_env(&env_id).await?;
            self.add_registered_env(&env_id, collection_id, env_spec.clone(), true)
                .await?;
        }

        self.env_collections
            .insert(String::from(collection_id), collection);
        Ok(())
    }

    pub async fn add_app_collection(
        &mut self,
        collection_id: &str,
        collection: Box<dyn AppCollection>,
        env_placement: Option<AppEnvPlacementStrategy>
    ) -> Result<()> {

        let placement_strategy = match env_placement {
            Some(strategy) => strategy,
            None => AppEnvPlacementStrategy::Default
        };

        for app_id in collection.get_app_ids().await {
            let app_spec = collection.get_app(&app_id).await?;

            let env_id: String = self.get_env_id_for_app(&app_id, app_spec, collection_id, &placement_strategy);

            self.add_registered_app(&app_id, app_spec.clone(), collection_id, env_id, true)
                .await?;
        }

        self.app_collections
            .insert(String::from(collection_id), collection);

        Ok(())
    }

    pub async fn list_envs(&self) -> &BTreeMap<String, VivaEnv> {
        &self.registered_envs
    }

    pub async fn sync_envs(&mut self, env_ids: &HashSet<String>) -> Result<()> {

        let mut missing: Vec<String> = vec![];

        let all_envs = self.get_env_ids().await;
        for env_name in env_ids {
            if ! all_envs.contains(&env_name) {
                missing.push(env_name.clone());
            }
        }
        match missing.len() {
            0 => {
                debug!("Syncing environments: {:?}", &env_ids);
            }
            _ => {
                bail!("The following environments are not registered: {:?}", missing);
            }
        }

        let mut env_ids_to_sync: Vec<String> = env_ids.into_iter().cloned().collect();
        if env_ids_to_sync.len() == 0 {
            env_ids_to_sync = self.get_env_ids().await;
        }

        for env_id in env_ids_to_sync {
            let env = self.get_env_mut(&env_id).await?;
            match env.sync_status {
                EnvSyncStatus::Unknown => {
                    println!("Syncing environment: {}", env_id);
                    env.check_and_update_sync_status();
                    match env.sync_status {
                        EnvSyncStatus::Synced => {
                            println!("Environment {} is already synced", env_id);
                        }
                        _ => {
                            println!("Syncing environment: {}", env_id);
                            env.sync().await?;
                        }
                    }
                }
                EnvSyncStatus::Synced => {
                    println!("Environment {} is already synced", env_id);
                }
                EnvSyncStatus::NotSynced => {
                    println!("Syncing environment: {}", env_id);
                    env.sync().await?;
                }
            }
        }
        Ok(())

    }

    pub async fn list_apps(&self) -> &BTreeMap<String, VivaApp> {

        &self.registered_apps

    }

    async fn create_env_instance(
        &self,
        env_id: &str,
        collection_id: String,
        env_spec: Option<VivaEnvSpec>,
    ) -> Result<VivaEnv> {
        let env_path = self.base_env_path.join(env_id);
        let env_spec_file: PathBuf = env_path.join(ENV_SPEC_FILENAME);
        let actual_env_spec: VivaEnvSpec = match env_spec_file.exists() {
            true => {
                let env_actual: VivaEnvSpec = read_model_spec(&env_spec_file).await?;
                env_actual
            }
            false => VivaEnvSpec {
                channels: vec![],
                pkg_specs: vec![],
            },
        };

        let env_spec = match env_spec {
            Some(spec) => spec,
            None => VivaEnvSpec::new(),
        };
        let viva_env: VivaEnv = VivaEnv::create(
            String::from(env_id),
            String::from(collection_id),
            env_spec,
            env_path,
            actual_env_spec,
            env_spec_file,
            EnvSyncStatus::Unknown,
        );

        Ok(viva_env)
    }

    async fn add_registered_app(
        &mut self,
        app_id: &str,
        app_spec: VivaAppSpec,
        collection_id: &str,
        env_id: String,
        allow_duplicate: bool,
    ) -> Result<bool> {
        match self.registered_apps.contains_key(app_id) {
            true => {
                if allow_duplicate {
                    debug!("Skipping duplicate app: {}",app_id);
                    Ok(false)
                } else {
                    Err(anyhow!("Duplicate app: {}", &app_id))
                }
            }
            false => {
                debug!("Registering app: {}", app_id);

                let app_instance = VivaApp::create(
                    String::from(app_id),
                    app_spec.clone(),
                    String::from(collection_id),
                    env_id
                );
                self.registered_apps
                    .insert(String::from(app_id), app_instance);
                Ok(true)
            }
        }

    }

    pub async fn merge_all_apps(&mut self) -> Result<()> {

        let app_ids: Vec<String> = self.get_app_ids().await;

        for app_id in app_ids {
            self.merge_app_into_env(&app_id).await?;
        }

        Ok(())
    }

    async fn merge_app_into_env(&mut self, app_id: &str) -> Result<()> {

        let app_env_spec = self.get_app(app_id).await?;
        let env_id = String::from(app_env_spec.get_env_id());
        let app_env_spec = app_env_spec.spec.env_spec.clone();
        let env = self.get_env_mut(&env_id).await?;

        env.merge_spec(&app_env_spec)?;

        Ok(())
    }

    async fn add_registered_env(
        &mut self,
        env_id: &str,
        collection_id: &str,
        env_spec: VivaEnvSpec,
        allow_duplicate: bool,
    ) -> Result<bool> {
        match self.registered_envs.contains_key(env_id) {
            true => {
                if allow_duplicate {
                    debug!("Skipping duplicate environment: {}", &env_id);
                    Ok(false)
                } else {
                    Err(anyhow!("Duplicate environment: {}", &env_id))
                }
            }
            false => {
                debug!("Registering environment: {}", &env_id);
                let env_instance = self
                    .create_env_instance(env_id, String::from(collection_id), Some(env_spec))
                    .await?;
                self.registered_envs
                    .insert(env_id.to_string(), env_instance);
                Ok(true)
            }
        }
    }

    pub async fn set_env_spec(&mut self, env_id: &str, env_spec: VivaEnvSpec) -> Result<()> {
        let col_id = &self.get_env(env_id).await?.collection_id.clone();
        let env_col = self
            .env_collections
            .get_mut(col_id)
            .expect(format!("Can't find env collection: {}", col_id).as_str());

        env_col.set_env(env_id, &env_spec).await?;
        Ok(())
    }

    pub async fn merge_env_specs(
        &mut self,
        target_env_id: &str,
        spec_to_merge: &VivaEnvSpec,
        update_env_spec: bool,
        add_if_not_exist: bool,
    ) -> Result<()> {
        if !self.has_env(target_env_id).await {
            if add_if_not_exist {
                // TODO: support other collections, not just 'default'
                self.add_registered_env(target_env_id, "default", spec_to_merge.clone(), false)
                    .await
                    .expect("Can't add env");
            } else {
                return Err(anyhow!(
                    "Can't merge environment, it does not exist (yet): {}",
                    target_env_id
                ));
            }
        }

        let env = self
            .get_env_mut(target_env_id)
            .await
            .expect("Can't get env");
        env.merge_spec(spec_to_merge)?;

        if update_env_spec {
            let updated_spec = env.spec.clone();
            self.set_env_spec(target_env_id, updated_spec).await?;
        }

        Ok(())
    }

    pub async fn get_env_ids(&self) -> Vec<String> {
        let mut env_ids = self
            .registered_envs
            .keys()
            .map(|x| x.to_string())
            .collect::<Vec<String>>();
        env_ids.sort();
        env_ids
    }

    pub async fn get_app_ids(&self) -> Vec<String> {
        let mut app_ids = self
            .registered_apps
            .keys()
            .map(|x| x.to_string())
            .collect::<Vec<String>>();
        app_ids.sort();
        app_ids
    }

    pub async fn has_env(&self, env_name: &str) -> bool {
        self.registered_envs.contains_key(env_name)
    }

    pub async fn add_app(
        &mut self,
        app_id: &str,
        app_spec: VivaAppSpec,
        collection_id: &str,
        placement_strategy: AppEnvPlacementStrategy
    ) -> Result<&VivaApp>{

        let env_id = self.get_env_id_for_app(app_id, &app_spec, collection_id, &placement_strategy);

        let app_col = self
            .app_collections
            .get_mut(collection_id)
            .expect(format!("App collection not found: {}", collection_id).as_str());

        app_col.set_app(app_id, &app_spec).await?;
        self.add_registered_app(app_id, app_spec, collection_id, env_id, false).await?;

        let app = self.get_app(app_id).await?;
        Ok(app)

    }

    pub async fn add_env(
        &mut self,
        env_id: &str,
        env: Option<VivaEnvSpec>,
        collection_id: Option<&str>,
    ) -> Result<&VivaEnv> {
        if self.has_env(env_id).await {
            return Err(anyhow!("Can't add environment: id '{}' already registered.", env_id));
        }

        let env_col_name = match collection_id {
            Some(col_name) => col_name,
            None => "default",
        };

        let env_col = self
            .env_collections
            .get_mut(env_col_name)
            .expect(format!("Environment collection not found: {}", env_col_name).as_str())
            .as_mut();

        let env_spec = match env {
            Some(env) => env,
            None => VivaEnvSpec::new(),
        };

        env_col.set_env(env_id, &env_spec).await?;

        self.add_registered_env(env_id, &env_col_name, env_spec, false)
            .await?;
        self.get_env(env_id).await
    }

    pub async fn get_app(&self, app_name: &str) -> Result<&VivaApp> {
        match self.registered_apps.get(app_name) {
            Some(app) => Ok(app),
            None => Err(anyhow!("App not found: {}", app_name)),
        }
    }

    pub async fn get_env(&self, env_name: &str) -> Result<&VivaEnv> {
        match self.registered_envs.get(env_name) {
            Some(env) => Ok(env),
            None => Err(anyhow!("Environment not found: {}", env_name)),
        }
    }

    pub async fn get_env_mut(&mut self, env_id: &str) -> Result<&mut VivaEnv> {

        match self.registered_envs.get_mut(env_id) {
            Some(env) => Ok(env),
            None => Err(anyhow!("Environment not found: {}", env_id)),
        }
    }

    /// Ensure the sync status of all viva envs is up to date.
    pub async fn check_envs_sync_status(&mut self) -> Result<()> {
        let env_ids = self.get_env_ids().await;
        for env_id in env_ids {
            let env = self.get_env_mut(&env_id).await?;
            if env.sync_status == EnvSyncStatus::Unknown {
                env.check_and_update_sync_status();
            }
        }
        Ok(())
    }

    pub fn get_env_id_for_app(&self, app_id: &str, _app_spec: &VivaAppSpec, collection_id: &str, placement_strategy: &AppEnvPlacementStrategy) -> String {

        match placement_strategy {
            AppEnvPlacementStrategy::Default => {
                String::from("default")
            },
            AppEnvPlacementStrategy::Custom(e_id) => { String::from(e_id) },
            AppEnvPlacementStrategy::CollectionId => {
                String::from(collection_id)
            },
            AppEnvPlacementStrategy::AppId => {
                String::from(app_id)
            },
        }
    }

    pub async fn remove_env(&mut self, env_id: &str) -> Result<()> {

        if ! self.has_env(&env_id).await {
            return Err(anyhow!("No environment registered with id '{}'.", env_id));
        }

        let env = self.get_env(&env_id).await?;

        let env_col_name = &env.collection_id.clone();

        let env_col = self
            .env_collections
            .get_mut(env_col_name)
            .expect(format!("Environment collection not found: {}", env_col_name).as_str())
            .as_mut();

        env_col.delete_env(env_id).await?;
        self.registered_envs.remove(env_id);
        let env_path = self.base_env_path.join(env_id);
        match env_path.exists() {
            true => {
                fs::remove_dir_all(env_path).await?;
            },
            false => {
                debug!("No environment path exists for env '{}', doing nothing.", env_id);
            }
        }


        Ok(())
    }

    pub async fn pretty_print_envs(&self) {
        let envs = self.list_envs().await;
        let mut env_names: Vec<String> = envs.keys().map(|k| k.to_string()).collect();
        env_names.sort();

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_titles(prettytable::row![
            "name", "path", "specs", "channels", "status"
        ]);

        let compact: bool = false;
        for env in env_names {
            if !compact {
                table.add_row(prettytable::row!["", "", "", "", ""]);
            }
            let viva_env = envs.get(&env).unwrap();
            let path = viva_env.get_env_path().to_str().unwrap();
            let specs = viva_env.spec.pkg_specs.join("\n");
            let channels = viva_env.spec.channels.join("\n");
            let status = &viva_env.sync_status;
            table.add_row(prettytable::row![env, path, specs, channels, status]);
        }
        table.printstd();
    }

    pub async fn pretty_print_apps(&self) {

        let apps = self.list_apps().await;
        let mut app_names: Vec<String> = apps.keys().map(|k| k.to_string()).collect();
        app_names.sort();

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_titles(prettytable::row![
            "name", "cmd", "pkg_specs", "channels", "env_id", "status"
        ]);

        let compact: bool = false;
        for app in app_names {
            if !compact {
                table.add_row(prettytable::row!["", "", "", "", "", ""]);
            }
            let viva_app = apps.get(&app).unwrap();
            let cmd = viva_app.spec.get_full_cmd().join(" ");
            let env_id = viva_app.get_env_id();
            let specs = viva_app.spec.env_spec.pkg_specs.join("\n");
            let channels = viva_app.spec.env_spec.channels.join("\n");
            let viva_env = self.get_env(&viva_app.get_env_id()).await.unwrap();
            // let specs = viva_env.spec.pkg_specs.join("\n");
            // let channels = viva_env.spec.channels.join("\n");
            let status = &viva_env.sync_status;

            table.add_row(prettytable::row![app, cmd, specs, channels, env_id, status]);
        }
        table.printstd();
    }
}
