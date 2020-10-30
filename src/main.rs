use anyhow::Context;
use chef::{OptimisationProfile, Recipe};
use clap::Clap;
use fs_err as fs;
use std::path::PathBuf;

/// Cache the dependencies of your Rust project.
#[derive(Clap)]
#[clap(
    bin_name = "cargo",
    version = "0.1",
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
    /// Build for the target triple.
    #[clap(long)]
    target: Option<String>,
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
            target,
        }) => {
            let profile = if release {
                OptimisationProfile::Release
            } else {
                OptimisationProfile::Debug
            };
            let serialized = fs::read_to_string(recipe_path)
                .context("Failed to read recipe from the specified path.")?;
            let recipe: Recipe =
                serde_json::from_str(&serialized).context("Failed to deserialize recipe.")?;
            recipe
                .cook(profile, target)
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
