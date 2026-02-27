//! Runs `cargo-chef` and `cargo` commands, and parses build output.

use assert_cmd::Command;
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command as StdCommand, Output};

pub(crate) fn run_prepare(project_dir: &Path, recipe_path: &Path, bin: Option<&str>) {
    let mut cmd = Command::cargo_bin("cargo-chef").unwrap();
    cmd.arg("chef")
        .arg("prepare")
        .arg("--recipe-path")
        .arg(recipe_path)
        .current_dir(project_dir);
    if let Some(bin) = bin {
        cmd.arg("--bin").arg(bin);
    }
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "prepare failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn run_cook(
    cook_dir: &Path,
    recipe_path: &Path,
    package: Option<&str>,
    profile: Option<&str>,
    target: Option<&str>,
    target_dir: Option<&str>,
) -> Output {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cmd = Command::cargo_bin("cargo-chef").unwrap();
    cmd.arg("chef")
        .arg("cook")
        .arg("--recipe-path")
        .arg(recipe_path)
        .env("CARGO", cargo)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(cook_dir);
    if let Some(profile) = profile {
        if profile == "release" {
            cmd.arg("--release");
        } else {
            cmd.arg("--profile").arg(profile);
        }
    }
    if let Some(package) = package {
        cmd.arg("--package").arg(package);
    }
    if let Some(target) = target {
        cmd.arg("--target").arg(target);
    }
    if let Some(target_dir) = target_dir {
        cmd.arg("--target-dir").arg(target_dir);
    }
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "cook failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub(crate) fn run_cargo_build(
    cook_dir: &Path,
    bin: Option<&str>,
    package: Option<&str>,
    profile: Option<&str>,
    target: Option<&str>,
    target_dir: Option<&str>,
) -> Output {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cmd = StdCommand::new(cargo);
    cmd.arg("build")
        .arg("--message-format")
        .arg("json")
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(cook_dir);
    if let Some(bin) = bin {
        cmd.arg("--bin").arg(bin);
    }
    if let Some(package) = package {
        cmd.arg("--package").arg(package);
    }
    if let Some(profile) = profile {
        if profile == "release" {
            cmd.arg("--release");
        } else {
            cmd.arg("--profile").arg(profile);
        }
    }
    if let Some(target) = target {
        cmd.arg("--target").arg(target);
    }
    if let Some(target_dir) = target_dir {
        cmd.arg("--target-dir").arg(target_dir);
    }
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "cargo build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub(crate) fn run_generate_lockfile(project_dir: &Path) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = StdCommand::new(cargo)
        .arg("generate-lockfile")
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(project_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo generate-lockfile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn run_cargo_update_precise(project_dir: &Path, dep: &str, version: &str) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = StdCommand::new(cargo)
        .arg("update")
        .arg("-p")
        .arg(dep)
        .arg("--precise")
        .arg(version)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(project_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo update -p {} --precise {} failed: {}",
        dep,
        version,
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn parse_compilation_output(output: &Output) -> (HashSet<String>, HashSet<String>) {
    let mut compiled = HashSet::new();
    let mut fresh = HashSet::new();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    for line in stdout.lines().chain(stderr.lines()) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("reason").and_then(|v| v.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let Some(package_id) = value.get("package_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = package_name(package_id);
        let is_fresh = value
            .get("fresh")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_fresh {
            fresh.insert(name);
        } else {
            compiled.insert(name);
        }
    }

    (compiled, fresh)
}

fn package_name(package_id: &str) -> String {
    if let Some(name) = package_id.split_whitespace().next() {
        if package_id.contains(" (") {
            return name.to_string();
        }
    }
    if package_id.starts_with("registry+") {
        if let Some((_prefix, rest)) = package_id.split_once('#') {
            if let Some((name, _version)) = rest.split_once('@') {
                return name.to_string();
            }
            return rest.to_string();
        }
    }
    if let Some(fragment) = package_id.rsplit('/').next() {
        if let Some((name, _version)) = fragment.split_once('#') {
            return name.to_string();
        }
        return fragment.to_string();
    }
    package_id.to_string()
}

pub(crate) fn cook_output_string(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("--- cook output ---\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}\n--- end cook output ---")
}
