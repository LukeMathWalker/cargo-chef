use anyhow::Context;
use chef::{CookArgs, DefaultFeatures, OptimisationProfile, Recipe, TargetArgs};
use clap::crate_version;
use clap::Clap;
use fs_err as fs;
use std::collections::HashSet;
use std::path::PathBuf;

/// Cache the dependencies of your Rust project.
#[derive(Clap)]
#[clap(
    bin_name = "cargo",
    version = crate_version!(),
    author = "Luca Palmieri <rust@lpalmieri.com>"
)]
pub struct Cli {
    #[clap(subcommand)]
    command: CargoInvocation,
}

#[derive(Clap)]
pub enum CargoInvocation {
    // All `cargo` subcommands receive their name (e.g. `chef` as the first command).
    // See https://github.com/rust-lang/rustfmt/pull/3569
    Chef {
        #[clap(subcommand)]
        command: Command,
    },
}

#[derive(Clap)]
pub enum Command {
    /// Analyze the current project to determine the minimum subset of files (Cargo.lock and
    /// Cargo.toml manifests) required to build it and cache dependencies.
    ///
    /// `cargo chef prepare` emits a recipe file that can be later used via
    /// `cargo chef cook --recipe <recipe-path>.json`.
    Prepare(Prepare),
    /// Re-hydrate the minimum project skeleton identified by `cargo chef prepare` and build
    /// it to cache dependencies.
    Cook(Cook),
}

#[derive(Clap)]
pub struct Prepare {
    /// The filepath used to save the computed recipe.
    ///
    /// It defaults to "recipe.json".
    #[clap(long, default_value = "recipe.json")]
    recipe_path: PathBuf,
}

#[derive(Clap)]
pub struct Cook {
    /// The filepath `cook` should be reading the recipe from.
    ///
    /// It defaults to "recipe.json".
    #[clap(long, default_value = "recipe.json")]
    recipe_path: PathBuf,
    /// Build in release mode.
    #[clap(long)]
    release: bool,
    /// Run `cargo check` instead of `cargo build`. Primarily useful for speeding up your CI pipeline.
    #[clap(long)]
    check: bool,
    /// Build for the target triple.
    #[clap(long)]
    target: Option<String>,
    /// Directory for all generated artifacts.
    #[clap(long, env = "CARGO_TARGET_DIR")]
    target_dir: Option<PathBuf>,
    /// Do not activate the `default` feature.
    #[clap(long)]
    no_default_features: bool,
    /// Space or comma separated list of features to activate.
    #[clap(long, use_delimiter = true, value_delimiter = ",")]
    features: Option<Vec<String>>,
    /// Build all benches
    #[clap(long)]
    benches: bool,
    /// Build all tests
    #[clap(long)]
    tests: bool,
    /// Build all examples
    #[clap(long)]
    examples: bool,
    /// Build all targets.
    /// This is equivalent to specifying `--tests --benches --examples`.
    #[clap(long)]
    all_targets: bool,
    /// Path to Cargo.toml
    #[clap(long)]
    manifest_path: Option<PathBuf>,
    /// Package to build (see `cargo help pkgid`)
    #[clap(long, short = 'p')]
    package: Option<String>,
    /// Build all members in the workspace.
    #[clap(long)]
    workspace: bool,
    /// Build offline.
    #[clap(long)]
    offline: bool,
    /// Cook using `#[no_std]` configuration  (does not affect `proc-macro` crates)
    #[clap(long)]
    no_std: bool,
}

fn _main() -> Result<(), anyhow::Error> {
    let current_directory = std::env::current_dir().unwrap();

    let cli = Cli::parse();
    // "Unwrapping" the actual command.
    let command = match cli.command {
        CargoInvocation::Chef { command } => command,
    };

    match command {
        Command::Cook(Cook {
            recipe_path,
            release,
            check,
            target,
            no_default_features,
            features,
            target_dir,
            benches,
            tests,
            examples,
            all_targets,
            manifest_path,
            package,
            workspace,
            offline,
            no_std,
        }) => {
            if atty::is(atty::Stream::Stdout) {
                eprintln!("WARNING stdout appears to be a terminal.");
                eprintln!(
                    "cargo-chef is not meant to be run in an interactive environment \
                and will overwrite some existing files (namely any `lib.rs`, `main.rs` and \
                `Cargo.toml` it finds)."
                );
                eprintln!();
                eprint!("To continue anyway, type `yes`: ");

                let mut answer = String::with_capacity(3);
                std::io::stdin()
                    .read_line(&mut answer)
                    .context("Failed to read from stdin")?;

                if "yes" != answer.trim() {
                    std::process::exit(1);
                }
            }

            let features: Option<HashSet<String>> = features.and_then(|features| {
                if features.is_empty() {
                    None
                } else {
                    Some(features.into_iter().collect())
                }
            });

            let profile = if release {
                OptimisationProfile::Release
            } else {
                OptimisationProfile::Debug
            };

            let default_features = if no_default_features {
                DefaultFeatures::Disabled
            } else {
                DefaultFeatures::Enabled
            };

            let serialized = fs::read_to_string(recipe_path)
                .context("Failed to read recipe from the specified path.")?;
            let recipe: Recipe =
                serde_json::from_str(&serialized).context("Failed to deserialize recipe.")?;
            let target_args = TargetArgs {
                benches,
                tests,
                examples,
                all_targets,
            };
            recipe
                .cook(CookArgs {
                    profile,
                    check,
                    default_features,
                    features,
                    target,
                    target_dir,
                    target_args,
                    manifest_path,
                    package,
                    workspace,
                    offline,
                    no_std,
                })
                .context("Failed to cook recipe.")?;
        }
        Command::Prepare(Prepare { recipe_path }) => {
            let recipe = Recipe::prepare(current_directory).context("Failed to compute recipe")?;
            let serialized =
                serde_json::to_string(&recipe).context("Failed to serialize recipe.")?;
            fs::write(recipe_path, serialized).context("Failed to save recipe to 'recipe.json'")?;
        }
    }
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    _main()
}
