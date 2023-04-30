use std::ffi::OsStr;

pub const DEFAULT_CHANNELS: [&'static str; 1] = ["conda-forge"];

#[cfg(windows)]
pub const CONDA_BIN_DIRNAME: &str = "Scripts";

#[cfg(unix)]
pub const CONDA_BIN_DIRNAME: &str = "bin";

pub const ENV_SPEC_FILENAME: &str = ".viva_env";
