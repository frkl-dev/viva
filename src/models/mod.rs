pub mod app;
pub mod cmd;
pub mod environment;

use crate::defaults::DEFAULT_CHANNELS;
// use directories::ProjectDirs;
use anyhow::{anyhow, Context, Error, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Result as SerdeJsonResult;
use serde_yaml::Result as SerdeYamlResult;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

pub(crate) async fn read_models_spec<T: DeserializeOwned>(
    specs_file: &PathBuf,
) -> Result<BTreeMap<String, T>> {
    match specs_file.exists() {
        true => {
            if specs_file.is_dir() {
                return Err(anyhow!(
                    "Can't parse spec file, path is a directory: {}",
                    specs_file.display()
                ));
            }
        }
        false => {
            return Err(anyhow!(
                "Specification file does not exist: {}",
                specs_file.display()
            ))
        }
    };

    let mut file = File::open(specs_file).await?;
    let mut specs_data = String::new();
    file.read_to_string(&mut specs_data).await?;

    match parse_models_spec(&specs_data) {
        Ok(envs_spec) => {
            return Ok(envs_spec);
        }
        Err(_) => {
            return Err(anyhow!(
                "Unable to parse specification file: {}",
                specs_file.display()
            ));
        }
    }
}

pub(crate) fn parse_models_spec<T: DeserializeOwned>(
    spec_string: &str,
) -> Result<BTreeMap<String, T>> {
    let json_result = parse_models_spec_json(spec_string);

    // TODO: check that alias is valid
    match json_result {
        Ok(env_spec) => {
            return Ok(env_spec);
        }
        Err(_) => {
            let yaml_result = parse_models_spec_yaml(spec_string);
            return yaml_result
                .with_context(|| format!("Unable to parse specification yaml: {}", spec_string));
        }
    }
}

pub(crate) fn parse_models_spec_json<'de, T: Deserialize<'de>>(
    env_spec_data: &'de str,
) -> Result<BTreeMap<String, T>> {
    let json_result: SerdeJsonResult<BTreeMap<String, T>> = serde_json::from_str(&env_spec_data);
    match json_result {
        Ok(env_spec) => Ok(env_spec),
        Err(_) => Err(anyhow!(
            "Unable to parse specification json: {}",
            env_spec_data
        )),
    }
}

pub(crate) fn parse_models_spec_yaml<T: DeserializeOwned>(
    env_spec_data: &str,
) -> Result<BTreeMap<String, T>> {
    let json_result: SerdeYamlResult<BTreeMap<String, T>> = serde_yaml::from_str(&env_spec_data);
    match json_result {
        Ok(env_spec) => Ok(env_spec),
        Err(_) => Err(anyhow!(
            "Unable to parse specification json: {}",
            env_spec_data
        )),
    }
}

/// Read model spec data from a file.
pub(crate) async fn read_model_spec<T: DeserializeOwned>(model_spec_file: &PathBuf) -> Result<T> {
    match model_spec_file.exists() {
        true => {
            if model_spec_file.is_dir() {
                return Err(anyhow!(
                    "Can't parse model specification path, it is a directory: {}",
                    model_spec_file.display()
                ));
            }
        }
        false => {
            return Err(anyhow!(
                "Specification file does not exist: {}",
                model_spec_file.display()
            ))
        }
    };

    let mut file = File::open(model_spec_file).await?;
    let mut env_spec_data = String::new();
    file.read_to_string(&mut env_spec_data).await?;

    match model_spec_file.extension() {
        Some(ext) => {
            if ext == "json" {
                return parse_model_spec_json(&env_spec_data);
            } else if ext == "yaml" || ext == "yml" {
                return parse_model_spec_yaml(&env_spec_data);
            } else {
                return Err(anyhow!(
                    "Unable to parse specification file, unknown extension: {}",
                    ext.to_string_lossy()
                ));
            }
        }
        None => {
            return parse_model_spec(&env_spec_data);
        }
    }
}

pub(crate) fn parse_model_spec_json<T: DeserializeOwned>(spec_string: &str) -> Result<T> {
    let json_result: SerdeJsonResult<T> = serde_json::from_str(&spec_string);
    match json_result {
        Ok(env_spec) => {
            return Ok(env_spec);
        }
        Err(_) => {
            return Err(anyhow!(
                "Unable to parse specification json: {}",
                spec_string
            ));
        }
    }
}

pub(crate) fn parse_model_spec_yaml<T: DeserializeOwned>(env_spec_data: &str) -> Result<T> {
    let json_result = serde_yaml::from_str(&env_spec_data);
    match json_result {
        Ok(env_spec) => {
            return Ok(env_spec);
        }
        Err(_) => {
            return Err(anyhow!(
                "Unable to parse specification yaml: {}",
                env_spec_data
            ));
        }
    }
}

pub(crate) fn parse_model_spec<T: DeserializeOwned>(env_spec_data: &str) -> Result<T> {
    let json_result = parse_model_spec_json(env_spec_data);

    // TODO: check that alias is valid
    match json_result {
        Ok(env_spec) => {
            return Ok(env_spec);
        }
        Err(_) => {
            let yaml_result = parse_model_spec_yaml(env_spec_data);
            return yaml_result
                .with_context(|| format!("Unable to parse specification: {}", env_spec_data));
        }
    }
}

pub(crate) async fn write_model_spec<T: Serialize>(
    model_spec_file: &PathBuf,
    model_spec: &T,
) -> Result<()> {
    if let Some(parent_dir) = model_spec_file.parent() {
        fs::create_dir_all(parent_dir).await?;
    }

    let mut file = File::create(model_spec_file).await?;
    let model_spec_data = serde_json::to_string(model_spec)?;
    file.write_all(model_spec_data.as_bytes()).await?;
    Ok(())
}
