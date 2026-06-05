# syntax=docker/dockerfile:1
#
# Example: Split Docker cache layers for third-party vs workspace-internal deps.
#
# ┌─────────────────────────────────────────────────────────────────────────────┐
# │ THE PROBLEM                                                                 │
# │                                                                             │
# │ In a large Cargo workspace the standard cargo-chef pattern has a            │
# │ disappointing property: the cook layer (which pre-compiles dependencies)    │
# │ is invalidated whenever *any* workspace manifest changes — adding a new     │
# │ internal crate, splitting one, adding a new binary target, etc. — even      │
# │ when no external (third-party) dependency changed at all.                   │
# │                                                                             │
# │ The root cause is that recipe.json captures both external deps AND the      │
# │ intra-workspace path dependencies. Any structural change to the workspace   │
# │ changes the recipe, which busts the cook cache, which forces a full         │
# │ recompile of *all* third-party crates from source.                          │
# │                                                                             │
# │ In a workspace with many internal packages (e.g. 26 in torrust-tracker),    │
# │ this means the cache is effectively always cold: every feature branch or    │
# │ PR that touches any manifest invalidates hundreds of MB of compiled crates. │
# └─────────────────────────────────────────────────────────────────────────────┘
#
# ┌─────────────────────────────────────────────────────────────────────────────┐
# │ THE SOLUTION                                                                │
# │                                                                             │
# │ Produce two recipes from the same workspace:                                │
# │                                                                             │
# │  recipe-thirdparty.json — all path deps stripped (external deps only).      │
# │    • Changes only when an external dependency is added/removed/updated.     │
# │    • The cook_thirdparty layer built from it is a durable cache of          │
# │      compiled third-party crates that survives any workspace-internal       │
# │      refactor.                                                              │
# │                                                                             │
# │  recipe.json — the standard full recipe (external + internal structure).    │
# │    • Changes when any manifest changes (normal cargo-chef behaviour).       │
# │    • The cook layer built on top of cook_thirdparty only needs to           │
# │      (re)compile the lightweight workspace stubs; all third-party           │
# │      artifacts are already present from the layer below.                    │
# │                                                                             │
# │ Stage graph:                                                                │
# │                                                                             │
# │   chef  ──►  planner                                                        │
# │                 │                                                           │
# │                 ├── recipe-thirdparty.json ──►  cook_thirdparty  (stable)   │
# │                 │                                      │                    │
# │                 └── recipe.json ──────────────────►  cook  (fast)           │
# │                                                         │                   │
# │                                  real source  ──────►  builder              │
# └─────────────────────────────────────────────────────────────────────────────┘
#
# Build:
#   docker build -t workspace-split-cache .
#
# Note: this Dockerfile uses the pre-built lukemathwalker/cargo-chef image.
# Adjust the tag to pin a specific cargo-chef + Rust version pair.

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

# strip_path_deps.py uses only the Python standard library (no third-party
# packages needed), but python3 itself may not be present on the slim Rust image.
# NOTE: This Dockerfile uses the Python workaround for illustration purposes.
# For the native solution (no Python required), see Dockerfile.native.
RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 \
 && rm -rf /var/lib/apt/lists/*

# ── planner ───────────────────────────────────────────────────────────────────
# Copies the full source tree and generates both recipe files.
# Nothing inside this stage changes the compiled output — it is purely
# analytical (cargo chef prepare only reads manifests, never compiles).
FROM chef AS planner
COPY . .

# Standard full recipe: captures external deps AND workspace-internal structure.
RUN cargo chef prepare --recipe-path recipe.json

# Thirdparty-only recipe: same as above but with all `path = "..."` deps stripped
# from every manifest and all local package entries removed from the lock file.
# This file is stable across any pure workspace-internal change.
RUN python3 strip_path_deps.py recipe.json recipe-thirdparty.json

# ── cook_thirdparty ───────────────────────────────────────────────────────────
# Compiles ONLY the external (third-party) crates.
#
# Cache behaviour:
#   • Re-runs when recipe-thirdparty.json changes, i.e. when an external
#     dependency is added, removed, or updated.
#   • Stays cached for any pure workspace-internal change: new crates, renamed
#     crates, restructured path deps, new binary targets, etc.
FROM chef AS cook_thirdparty
COPY --from=planner /app/recipe-thirdparty.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# ── cook ──────────────────────────────────────────────────────────────────────
# Inherits all third-party artifacts from cook_thirdparty.
# Runs cargo chef cook with the full recipe to build the workspace stubs.
# Cargo reuses every external crate already compiled in cook_thirdparty and
# only (re)compiles the lightweight workspace-member stub files.
#
# Cache behaviour:
#   • Re-runs when recipe.json changes (any manifest change).
#   • But third-party compilation is skipped — only stubs are (re)built, which
#     is fast even for large workspaces.
FROM cook_thirdparty AS cook
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# ── builder ───────────────────────────────────────────────────────────────────
# Copies real application source over the skeleton and compiles the actual app.
# Third-party artifacts are reused from cook; only workspace crates are rebuilt.
FROM cook AS builder
COPY . .
RUN cargo build --release --bin app

# ── runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/app /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/app"]
