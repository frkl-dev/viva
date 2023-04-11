// use std::error::Error;
// use std::fmt;
// use std::path::PathBuf;
//
// #[derive(Debug)]
// pub(crate) struct InvalidFileTypeError {
//     path: PathBuf,
//     details: String,
// }
//
// impl InvalidFileTypeError {
//     pub(crate) fn new(path: PathBuf, msg: &str) -> InvalidFileTypeError {
//         InvalidFileTypeError {
//             path: path,
//             details: msg.to_string(),
//         }
//     }
// }
//
// impl fmt::Display for InvalidFileTypeError {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "{}", self.details)
//     }
// }
// impl Error for InvalidFileTypeError {
//     fn description(&self) -> &str {
//         &self.details
//     }
// }
