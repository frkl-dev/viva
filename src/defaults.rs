use directories::ProjectDirs;

/// a struct that holds the global app configuration
#[derive(Debug)]
pub struct VivaGlobals {
    pub qualifier: String,
    pub organization: String,
    pub application: String,
    pub project_dirs: ProjectDirs
}


impl VivaGlobals {

    #[allow(dead_code)]
    pub fn clone(&self) -> VivaGlobals {
        VivaGlobals {
            qualifier: self.qualifier.clone(),
            organization: self.organization.clone(),
            application: self.application.clone(),
            project_dirs: self.project_dirs.clone()
        }
    }

    /// create a new Globals struct for the viva library
    pub fn new() -> VivaGlobals {

        let project_dirs = ProjectDirs::from("dev", "frkl", "viva").expect("Cannot create project directories");
        VivaGlobals {
            qualifier: String::from("dev"),
            organization: String::from("frkl"),
            application: String::from("viva"),
            project_dirs: project_dirs
        }
    }

    /// create a new Globals struct for a 3rd party application
    #[allow(dead_code)]
    pub fn create(qualifier: &str, organization: &str, application: &str) -> VivaGlobals {
        let project_dirs = ProjectDirs::from(qualifier, organization, application).expect("Cannot create project directories");
        VivaGlobals {
            qualifier: String::from(qualifier),
            organization: String::from(organization),
            application: String::from(application),
            project_dirs: project_dirs
        }
    }

}

pub const DEFAULT_CHANNELS: [&'static str; 1] = ["conda-forge"];

#[cfg(windows)]
pub const CONDA_BIN_DIRNAME: &str = "Scripts";

#[cfg(unix)]
pub const CONDA_BIN_DIRNAME: &str = "bin";

