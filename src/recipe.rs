use crate::Skeleton;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub skeleton: Skeleton,
}

impl Recipe {
    pub fn prepare(base_path: PathBuf) -> Result<Self, anyhow::Error> {
        let skeleton = Skeleton::derive(&base_path)?;
        Ok(Recipe { skeleton })
    }

    pub fn cook(
        &self,
        profile: OptimisationProfile,
        target: Option<String>,
    ) -> Result<(), anyhow::Error> {
        self.skeleton.build_minimum_project()?;
        build_dependencies(profile, target);

        let current_directory = std::env::current_dir()?;
        self.skeleton
            .remove_compiled_dummy_libraries(current_directory, profile)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OptimisationProfile {
    Release,
    Debug,
}

fn build_dependencies(profile: OptimisationProfile, target: Option<String>) {
    let mut command = Command::new("cargo");
    let command_with_args = command.arg("build");
    if profile == OptimisationProfile::Release {
        command_with_args.arg("--release");
    }
    if let Some(target) = target {
        command_with_args.arg("--target").arg(target);
    }

    execute_command(command_with_args);
}

fn execute_command(command: &mut Command) {
    let mut child = command
        .envs(std::env::vars())
        .spawn()
        .expect("Failed to execute process");

    let exit_status = child.wait().expect("Failed to run command");

    if !exit_status.success() {
        match exit_status.code() {
            Some(code) => panic!("Exited with status code: {}", code),
            None => panic!("Process terminated by signal"),
        }
    }
}
