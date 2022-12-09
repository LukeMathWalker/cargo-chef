<h1 align="center">cargo-chef</h1>
<div align="center">
 <strong>
   Cache the dependencies of your Rust project and speed up your Docker builds.
 </strong>
</div>

<br />

<div align="center">
  <!-- Crates version -->
  <a href="https://crates.io/crates/cargo-chef">
    <img src="https://img.shields.io/crates/v/cargo-chef.svg?style=flat-square"
    alt="Crates.io version" />
  </a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/cargo-chef">
    <img src="https://img.shields.io/crates/d/cargo-chef.svg?style=flat-square"
      alt="Download" />
  </a>
</div>
<br/>

# Table of Contents
0. [How to install](#how-to-install)
1. [How to use](#how-to-use)
2. [Benefits vs Limitations](#benefits-vs-limitations)
3. [License](#license)

## How To Install 

You can install `cargo-chef` from [crates.io](https://crates.io) with

```bash
cargo install cargo-chef --locked
```

## How to use

> :warning:  **cargo-chef is not meant to be run locally**  
> Its primary use-case is to speed up container builds by running BEFORE
> the actual source code is copied over. Don't run it on existing codebases to avoid
> having files being overwritten.

`cargo-chef` exposes two commands: `prepare` and `cook`:

```bash
cargo chef --help
```

```text

cargo-chef

USAGE:
    cargo chef <SUBCOMMAND>

SUBCOMMANDS:
    cook       Re-hydrate the minimum project skeleton identified by `cargo chef prepare` and
               build it to cache dependencies
    prepare    Analyze the current project to determine the minimum subset of files (Cargo.lock
               and Cargo.toml manifests) required to build it and cache dependencies
```

`prepare` examines your project and builds a _recipe_ that captures the set of information required to build your dependencies.

```bash
cargo chef prepare --recipe-path recipe.json
```

Nothing too mysterious going on here, you can examine the `recipe.json` file: it contains the skeleton of your project (e.g. all the `Cargo.toml` files with their relative path, the `Cargo.lock` file is available) plus a few additional pieces of information.  
 In particular it makes sure that all libraries and binaries are explicitly declared in their respective `Cargo.toml` files even if they can be found at the canonical default location (`src/main.rs` for a binary, `src/lib.rs` for a library).
 
The `recipe.json` is the equivalent of the Python `requirements.txt` file - it is the only input required for `cargo chef cook`, the command that will build out our dependencies:

```bash
cargo chef cook --recipe-path recipe.json
```

If you want to build in `--release` mode:

```bash
cargo chef cook --release --recipe-path recipe.json
```

You can also choose to override which Rust toolchain should be used. E.g., to force the `nightly` toolchain:

```bash
cargo +nightly chef cook --recipe-path recipe.json
```

`cargo-chef` is designed to be leveraged in Dockerfiles:

```dockerfile
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN --mount=type=cache,target=/usr/local/cargo/registry cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin app

# We do not need the Rust toolchain to run the binary!
FROM debian:buster-slim AS runtime
WORKDIR app
COPY --from=builder /app/target/release/app /usr/local/bin
ENTRYPOINT ["/usr/local/bin/app"]
```

We are using three stages: the first computes the recipe file, the second caches our dependencies and builds the binary, the third is our runtime environment.  
As long as your dependencies do not change the `recipe.json` file will stay the same, therefore the outcome of `cargo cargo chef cook --release --recipe-path recipe.json` will be cached, massively speeding up your builds (up to 5x measured on some commercial projects).

### Pre-built images

We offer `lukemathwalker/cargo-chef` as a pre-built Docker image equipped with both Rust and `cargo-chef`.

The tagging scheme is `<cargo-chef version>-rust-<rust version>`.  
For example, `0.1.22-rust-1.56.0`.  
You can choose to get the latest version of either `cargo-chef` or `rust` by using:
- `latest-rust-1.56.0` (use latest `cargo-chef` with specific Rust version);
- `0.1.22-rust-latest` (use latest Rust with specific `cargo-chef` version).
You can find [all the available tags on Dockerhub](https://hub.docker.com/r/lukemathwalker/cargo-chef).

> :warning:  **You must use the same Rust version in all stages**  
> If you use a different Rust version in one of the stages
> caching will not work as expected.
  
### Without the pre-built image

If you do not want to use the `lukemathwalker/cargo-chef` image, you can simply install the CLI within the Dockerfile:

```dockerfile
FROM rust:1 AS chef 
# We only pay the installation cost once, 
# it will be cached from the second build onwards
RUN cargo install cargo-chef 
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin app

# We do not need the Rust toolchain to run the binary!
FROM debian:buster-slim AS runtime
WORKDIR app
COPY --from=builder /app/target/release/app /usr/local/bin
ENTRYPOINT ["/usr/local/bin/app"]
```

### Running the binary in Alpine

If you want to run your application using the `alpine` distribution you need to create a fully static binary.  
The recommended approach is to build for the `x86_64-unknown-linux-musl` target using [`muslrust`](https://github.com/clux/muslrust).  
`cargo-chef` works for `x86_64-unknown-linux-musl`, but we are **cross-compiling** - the target
toolchain must be explicitly specified.

A sample Dockerfile looks like this:

```dockerfile
# Using the `rust-musl-builder` as base image, instead of 
# the official Rust toolchain
FROM clux/muslrust:stable AS chef
USER root
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Notice that we are specifying the --target flag!
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry cargo build --release --target x86_64-unknown-linux-musl --bin app

FROM alpine AS runtime
RUN addgroup -S myuser && adduser -S myuser -G myuser
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/app /usr/local/bin/
USER myuser
CMD ["/usr/local/bin/app"]
```

## Benefits vs Limitations

`cargo-chef` has been tested on a few OpenSource projects and some of commercial projects, but our testing has definitely not exhausted the range of possibilities when it comes to `cargo build` customisations and we are sure that there are a few rough edges that will have to be smoothed out - please file issues on [GitHub](https://github.com/LukeMathWalker/cargo-chef).

### Benefits of `cargo-chef`:

A common alternative is to load a minimal `main.rs` into a container with `Cargo.toml` and `Cargo.lock` to build a Docker layer that consists of only your dependencies ([more info here](https://www.lpalmieri.com/posts/fast-rust-docker-builds/#caching-rust-builds)). This is fragile compared to `cargo-chef` which will instead:

- automatically pick up all crates in a workspace (and new ones as they are added)
- keep working when files or crates are moved around, which would instead require manual edits to the `Dockerfile` using the "manual" approach
- generate fewer intermediate Docker layers (for workspaces)

### Limitations and caveats:

- `cargo cook` and `cargo build` must be executed from the same working directory. If you examine the `*.d` files under `target/debug/deps` for one of your projects using `cat` you will notice that they contain absolute paths referring to the project `target` directory. If moved around, `cargo` will not leverage them as cached dependencies;
- `cargo build` will build local dependencies (outside of the current project) from scratch, even if they are unchanged, due to the reliance of its fingerprinting logic on timestamps (see [this _long_ issue on `cargo`'s repository](https://github.com/rust-lang/cargo/issues/2644));

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
