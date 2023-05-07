use anyhow::{anyhow, Context, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Result as SerdeJsonResult;
use serde_yaml::Result as SerdeYamlResult;

mod config;
mod context;
mod defaults;
mod errors;
pub mod models;
mod rattler;
mod status;

extern crate prettytable;

pub use crate::rattler::global_multi_progress;
pub use crate::rattler::writer::IndicatifWriter;
pub use defaults::DEFAULT_CHANNELS;

pub use crate::context::VivaContext;
pub use crate::models::environment::VivaEnvSpec;
