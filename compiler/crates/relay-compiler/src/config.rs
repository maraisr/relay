/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::compiler_state::{ProjectName, SourceSetName};
use crate::errors::{ConfigValidationError, Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// The full compiler config. This is a combination of:
/// - the configuration file
/// - the absolute path to the root of the compiled projects
/// - TODO: injected code to produce additional files
#[derive(Debug)]
pub struct Config {
    /// Root directory of all projects to compile. Any other paths in the
    /// compiler should be relative to this root unless otherwise noted.
    pub root_dir: PathBuf,
    pub sources: HashMap<PathBuf, SourceSetName>,
    pub blacklist: Vec<String>,
    pub projects: HashMap<ProjectName, ConfigProject>,
}
impl Config {
    pub fn load(root_dir: PathBuf, config_path: PathBuf) -> Result<Self> {
        let config_string =
            std::fs::read_to_string(&config_path).map_err(|err| Error::ConfigFileRead {
                config_path: config_path.clone(),
                source: err,
            })?;
        let config_file: ConfigFile =
            serde_json::from_str(&config_string).map_err(|err| Error::ConfigFileParse {
                config_path: config_path.clone(),
                source: err,
            })?;
        let config = Self {
            root_dir,
            sources: config_file.sources,
            blacklist: config_file.blacklist,
            projects: config_file.projects,
        };
        let validation_errors = config.validate(true);
        if validation_errors.is_empty() {
            Ok(config)
        } else {
            Err(Error::ConfigFileValidation {
                config_path: config_path.clone(),
                validation_errors,
            })
        }
    }

    /// Loads a config file without validation for use in tests.
    #[cfg(test)]
    pub fn from_string_for_test(config_string: &str) -> Result<Self> {
        let config_file: ConfigFile =
            serde_json::from_str(&config_string).expect("Failed to deserialize ConfigFile.");
        let config = Self {
            root_dir: "/virtual/root".into(),
            sources: config_file.sources,
            blacklist: config_file.blacklist,
            projects: config_file.projects,
        };
        let validation_errors = config.validate(false);
        if !validation_errors.is_empty() {
            panic!("Found validation ")
        }
        Ok(config)
    }

    /// `validate_fs` enables filesystem checks for existence of paths
    fn validate(&self, validate_fs: bool) -> Vec<ConfigValidationError> {
        let mut errors = Vec::new();

        if validate_fs {
            if !self.root_dir.is_dir() {
                errors.push(ConfigValidationError::RootNotDirectory {
                    root_dir: self.root_dir.clone(),
                });
                // early return, no point in continuing validation
                return errors;
            }

            // each source should point to an existing directory
            for source_dir in self.sources.keys() {
                let abs_source_dir = self.root_dir.join(source_dir);
                if !abs_source_dir.exists() {
                    errors.push(ConfigValidationError::SourceNotExistent {
                        source_dir: abs_source_dir.clone(),
                    });
                } else if !abs_source_dir.is_dir() {
                    errors.push(ConfigValidationError::SourceNotDirectory {
                        source_dir: abs_source_dir.clone(),
                    });
                }
            }
        }

        // each project should have at least one source
        for &project_name in self.projects.keys() {
            if !self
                .sources
                .values()
                .any(|source_set_name| *source_set_name == project_name.as_source_set_name())
            {
                errors.push(ConfigValidationError::ProjectWithoutSource { project_name });
            }
        }

        errors
    }
}

/// Schema of the compiler configuration JSON file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    /// A mapping from directory paths (relative to the root) to a source set.
    /// If a path is a subdirectory of another path, the more specific path
    /// wins.
    sources: HashMap<PathBuf, SourceSetName>,

    /// Glob patterns that should not be part of the sourcces even if they are
    /// in the source set directories.
    #[serde(default)]
    blacklist: Vec<String>,

    /// Configuration of projects to compile.
    projects: HashMap<ProjectName, ConfigProject>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigProject {
    /// Additional source sets that can be referenced from this project, but
    /// are not producing outputs. Another project should be setup to produce
    /// them.
    #[serde(default)]
    pub base: Vec<SourceSetName>,

    /// A project without an output directory will put the generated files in
    /// a __generated__ directory next to the input file.
    /// All files in these directories should be generated by the Relay
    /// compiler, so that the compiler can cleanup extra files.
    #[serde(default)]
    pub output: Option<PathBuf>,

    /// Directory containing *.graphql files with schema extensions.
    #[serde(default)]
    pub extensions: Vec<PathBuf>,

    /// Path to the schema.
    pub schema: PathBuf,
}