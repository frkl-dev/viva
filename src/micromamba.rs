// use crate::defaults::Globals;
// use crate::errors::InvalidFileTypeError;
// use bzip2::read::BzDecoder;
// use is_executable::IsExecutable;
// use std::fs::create_dir_all;
// use std::path::{Path, PathBuf};
// use std::process::Command;
// use tar::Archive;
//
// type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
//
// struct CondaEnvDesc {
//     channels: Vec<String>,
//     dependencies: Vec<String>,
// }
//
// pub(crate) async fn ensure_micromamba(globals: &Globals) -> Result<PathBuf> {
//     let bin_path = globals.project_dirs().data_dir().join("bin");
//     let mut exe_path = bin_path.join("micromamba");
//
//     if exe_path.is_executable() {
//         return Ok(exe_path);
//     }
//
//     let url = String::from("https://micro.mamba.pm/api/micromamba/linux-64/latest");
//
//     let resp = reqwest::get(url).await?.bytes().await?;
//     let tarfile = BzDecoder::new(resp.as_ref());
//
//     let mut archive = Archive::new(tarfile);
//     if !bin_path.exists() {
//         create_dir_all(bin_path);
//     }
//
//     for (i, file) in archive.entries().unwrap().enumerate() {
//         let mut file = file.unwrap();
//         match file.path().unwrap().to_str().unwrap() {
//             "bin/micromamba" => {
//                 file.unpack(&exe_path);
//             }
//             _ => {}
//         }
//     }
//
//     return Ok(exe_path);
// }

// pub(crate) async fn create_conda_env(env_name: &str, globals: &Globals) -> Result<PathBuf> {
//     let env_path = &globals.get_default_env_path(env_name);
//     println!("env_path: {:?}", env_path);
//     if env_path.exists() {
//         return Ok(env_path.to_path_buf());
//     }
//     // TODO check env validity
//
//     let path = ensure_micromamba(globals).await.unwrap();
//     println!("Creating conda environment: {}", env_name);
//     let output = Command::new(path)
//         .arg("create")
//         .arg("-p")
//         .arg(env_path.as_path())
//         .arg("-c")
//         .arg("conda-forge")
//         .arg("-c")
//         .arg("dharpa")
//         .arg("-y")
//         .arg("python=3.10")
//         .arg("kiara")
//         .output()
//         .expect("failed to execute process");
//     println!("output: {:?}", output);
//
//     Ok(env_path.to_path_buf())
// }

// pub(crate) async fn ensure_kiara_env(env_name: &str, globals: &Globals) -> Result<PathBuf> {
//     let kiara_bin_path = globals.get_default_env_path(env_name);
//
//     if kiara_bin_path.exists() {
//         if !kiara_bin_path.is_executable() {
//             return Err(Box::new(InvalidFileTypeError::new(
//                 kiara_bin_path,
//                 "Not executable",
//             )));
//         }
//         return Ok(kiara_bin_path);
//     }
//
//     println!("No kiara environment: {}", env_name);
//     let env_path = create_conda_env(env_name, globals).await;
//     return env_path;
// }
