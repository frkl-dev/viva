use directories::ProjectDirs;

/// a struct that holds the global app configuration
#[derive(Debug)]
pub struct Globals {
    pub qualifier: String,
    pub organization: String,
    pub application: String,
    pub project_dirs: ProjectDirs
}


impl Globals {

    #[allow(dead_code)]
    pub(crate) fn clone(&self) -> Globals {
        Globals {
            qualifier: self.qualifier.clone(),
            organization: self.organization.clone(),
            application: self.application.clone(),
            project_dirs: self.project_dirs.clone()
        }
    }

    /// create a new Globals struct for the viva library
    pub(crate) fn new() -> Globals {

        let project_dirs = ProjectDirs::from("dev", "frkl", "viva").expect("Cannot create project directories");
        Globals {
            qualifier: String::from("dev"),
            organization: String::from("frkl"),
            application: String::from("viva"),
            project_dirs: project_dirs
        }
    }

    /// create a new Globals struct for a 3rd party application
    #[allow(dead_code)]
    pub fn create(qualifier: &str, organization: &str, application: &str) -> Globals {
        let project_dirs = ProjectDirs::from(qualifier, organization, application).expect("Cannot create project directories");
        Globals {
            qualifier: String::from(qualifier),
            organization: String::from(organization),
            application: String::from(application),
            project_dirs: project_dirs
        }
    }

}

pub const DEFAULT_CHANNELS: [&'static str; 1] = ["conda-forge"];

// impl EnvDetails {
//     pub fn new(env_name: &str, globals: &Globals, match_spec: Vec<MatchSpec>) -> EnvDetails {
//         EnvDetails {
//             env_name: String::from(env_name),
//             env_prefix: globals.env_prefix.join(env_name),
//             match_spec: match_spec,
//         }
//     }
// }
