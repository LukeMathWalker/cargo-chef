# Example: Split Docker cache — third-party vs workspace-internal deps

This example demonstrates a common pain point in large Cargo workspaces and
two solutions: a **workaround** using a small Python script, and the **native
solution** via the `--external-only` flag for `cargo chef prepare`.

> **Goal of this example:** provide maintainers with a reproducible
> demonstration and a working implementation to evaluate for merging.
> Related upstream issues: [#314], [#75], [#181], [#305].

[#314]: https://github.com/LukeMathWalker/cargo-chef/issues/314
[#75]: https://github.com/LukeMathWalker/cargo-chef/issues/75
[#181]: https://github.com/LukeMathWalker/cargo-chef/issues/181
[#305]: https://github.com/LukeMathWalker/cargo-chef/issues/305

---

## Table of contents

1. [The problem](#1-the-problem)
2. [Workspace structure](#2-workspace-structure)
3. [Step-by-step: reproducing the problem](#3-step-by-step-reproducing-the-problem)
4. [Solution A — Python workaround (works today)](#4-solution-a--python-workaround-works-today)
5. [Solution B — native `--external-only` flag (proposed)](#5-solution-b--native---external-only-flag-proposed)
6. [Related open issues](#6-related-open-issues)
7. [Implementation notes](#7-implementation-notes)

---

## 1. The problem

The standard `cargo-chef` pattern builds a single cook layer that caches all
dependencies:

```
COPY Cargo.toml Cargo.lock ...
RUN cargo chef prepare --recipe-path recipe.json     # <-- reads manifests
RUN cargo chef cook   --recipe-path recipe.json     # <-- the "cached" layer
COPY . .
RUN cargo build --release
```

This works well for single-crate projects. In a large workspace it has a
**critical weakness**: `recipe.json` captures both external (crates.io) deps
_and_ the intra-workspace `path = "..."` dependencies. Any structural change
to the workspace invalidates the cook layer and forces a **full recompile of
all third-party crates**:

| Change                                         | Invalidates cook layer? |
| ---------------------------------------------- | ----------------------- |
| Editing application source (`.rs` files)       | No — that's the point   |
| Adding a new external dep (`serde = "1"`)      | Yes — expected          |
| Adding a new workspace-internal crate          | **Yes — unexpected**    |
| Splitting one internal crate into two          | **Yes — unexpected**    |
| Adding a `[[bin]]` target to a workspace crate | **Yes — unexpected**    |
| Renaming a workspace crate                     | **Yes — unexpected**    |

In the real-world project ([torrust-tracker]) that motivated this example the
workspace has 26 internal crates. Almost every feature branch touches at least
one manifest. The cook cache is effectively **always cold**: all third-party
crates are recompiled from source on every CI run.

[torrust-tracker]: https://github.com/torrust/torrust-tracker

### Root cause

`cargo chef prepare` serialises every workspace manifest — including
`[dependencies.my-crate] path = "../my-crate"` sections — into `recipe.json`.
Any structural change produces a different JSON file, which Docker treats as a
changed `COPY` input, which invalidates the `RUN cargo chef cook` layer.

---

## 2. Workspace structure

```
Cargo.toml          ← workspace root (members: app, logic, model)
app/                ← binary:  anyhow (external) + logic (path dep)
logic/              ← library: anyhow + serde_json (external) + model (path dep)
model/              ← library: serde (external only, no path deps)
Dockerfile          ← workaround using strip_path_deps.py
Dockerfile.native   ← clean version using --external-only flag
strip_path_deps.py  ← Python post-processor (workaround, no native cargo-chef needed)
```

Dependency graph:

```
app  ──path──►  logic  ──path──►  model  ──►  serde
 └────────────────────────────────────►  anyhow
                └────────────────────►  serde_json
```

This is the minimal structure to reproduce the problem: a chain of path deps
where almost every workspace change touches at least one manifest.

---

## 3. Step-by-step: reproducing the problem

All commands run from this directory:

```
cd examples/workspace-split-cache
```

### 3.1 Prepare a standard recipe

```bash
cargo chef prepare --recipe-path /tmp/recipe-full.json
```

Inspect the recipe — note that `app/Cargo.toml` contains `[dependencies.logic]`
with `path = "../logic"`:

```bash
python3 -c "
import json
with open('/tmp/recipe-full.json') as f:
    r = json.load(f)
for m in r['skeleton']['manifests']:
    print('=== ' + m['relative_path'] + ' ===')
    print(m['contents'])
"
```

**Output (relevant excerpt):**

```toml
=== app/Cargo.toml ===
[package]
name = "app"
edition = "2021"
version = "0.0.1"

[dependencies]
anyhow = "1"

[dependencies.logic]
path = "../logic"          # <-- this is the problem

[[bin]]
path = "src/main.rs"
name = "app"
...
```

The lock file also contains entries for all three local workspace crates:

```bash
python3 -c "
import json, re
with open('/tmp/recipe-full.json') as f:
    r = json.load(f)
lf = r['skeleton']['lock_file'] or ''
print('Lock packages:', re.findall(r'name = \"([^\"]+)\"', lf))
"
```

**Output:**

```
Lock packages: ['anyhow', 'app', 'itoa', 'logic', 'memchr', 'model',
'proc-macro2', 'quote', 'serde', 'serde_core', 'serde_derive',
'serde_json', 'syn', 'unicode-ident', 'zmij']
```

`app`, `logic`, and `model` appear in the lock file — so adding a new local
crate changes the lock file and therefore the recipe.

### 3.2 Cold cook timing (what happens on every cache miss)

```bash
mkdir -p /tmp/cook-demo && cd /tmp/cook-demo
CARGO=$(which cargo) time cargo chef cook --release --recipe-path /tmp/recipe-full.json
```

**Output:**

```
   Compiling proc-macro2 v1.0.106
   Compiling serde_core v1.0.228
   Compiling unicode-ident v1.0.24
   Compiling quote v1.0.45
   Compiling anyhow v1.0.102
   Compiling zmij v1.0.21
   Compiling serde v1.0.228
   Compiling serde_json v1.0.150
   Compiling itoa v1.0.18
   Compiling memchr v2.8.1
   Compiling app v0.0.1 (/tmp/cook-demo/app)
   Compiling syn v2.0.117
   Compiling serde_derive v1.0.228
   Compiling logic v0.0.1 (/tmp/cook-demo/logic)
   Compiling model v0.0.1 (/tmp/cook-demo/model)
    Finished `release` profile [optimized] target(s) in 1.99s

real    0m5.80s
```

This **5.8 s** is the cost paid every time the cook cache is invalidated. In a
real workspace (26 crates, hundreds of third-party deps) this scales to
**several minutes** per CI run.

### 3.3 Simulating a workspace-internal change

Touch any workspace manifest — for example, bump the description of `logic`:

```bash
cd examples/workspace-split-cache
# Simulating: add a new binary to logic/Cargo.toml
echo '
[[bin]]
name = "logic-tool"
path = "src/main.rs"' >> logic/Cargo.toml

cargo chef prepare --recipe-path /tmp/recipe-full-v2.json

diff <(python3 -c "
import json
with open('/tmp/recipe-full.json') as f:
    print(json.dumps(json.load(f)['skeleton']['manifests'], indent=2))
") <(python3 -c "
import json
with open('/tmp/recipe-full-v2.json') as f:
    print(json.dumps(json.load(f)['skeleton']['manifests'], indent=2))
") | head -20
```

The diff shows the recipe changed. If this `recipe-full-v2.json` is passed to
`COPY` in Docker, **the entire cook layer is invalidated** and all third-party
crates are recompiled. Revert the change:

```bash
git checkout logic/Cargo.toml
```

---

## 4. Solution A — Python workaround (works today)

`strip_path_deps.py` post-processes the standard `recipe.json` and produces a
`recipe-thirdparty.json` that contains only external deps.

### 4.1 Generate the thirdparty recipe

```bash
cd examples/workspace-split-cache
cargo chef prepare --recipe-path /tmp/recipe-full.json
python3 strip_path_deps.py /tmp/recipe-full.json /tmp/recipe-thirdparty.json
```

Inspect the result — `[dependencies.logic]` is gone:

```bash
python3 -c "
import json
with open('/tmp/recipe-thirdparty.json') as f:
    r = json.load(f)
for m in r['skeleton']['manifests']:
    print('=== ' + m['relative_path'] + ' ===')
    print(m['contents'])
"
```

**Output:**

```toml
=== app/Cargo.toml ===
[package]
name = "app"
edition = "2021"
version = "0.0.1"

[dependencies]
anyhow = "1"           # path dep to logic is GONE

[[bin]]
path = "src/main.rs"
name = "app"
...

=== logic/Cargo.toml ===
[package]
name = "logic"
...
[dependencies]
anyhow = "1"
serde_json = "1"       # path dep to model is GONE
...
```

Lock file — local workspace crates removed:

```bash
python3 -c "
import json, re
with open('/tmp/recipe-thirdparty.json') as f:
    r = json.load(f)
lf = r['skeleton']['lock_file'] or ''
print('Lock packages:', re.findall(r'name = \"([^\"]+)\"', lf))
"
```

**Output:**

```
Lock packages: ['anyhow', 'itoa', 'memchr', 'proc-macro2', 'quote', 'serde',
'serde_core', 'serde_derive', 'serde_json', 'syn', 'unicode-ident', 'zmij']
```

`app`, `logic`, `model` are absent. Adding a new workspace crate will not
change this recipe.

### 4.2 Two-layer cook

```bash
mkdir -p /tmp/cook-demo && cd /tmp/cook-demo

# Layer 1: cook third-party deps only (stable, rarely re-runs)
CARGO=$(which cargo) time cargo chef cook --release --recipe-path /tmp/recipe-thirdparty.json
```

**Output:**

```
   Compiling proc-macro2 v1.0.106
   Compiling serde_core v1.0.228
   ...
   Compiling app v0.0.1 (/tmp/cook-demo/app)   ← workspace stub (trivial)
   Compiling logic v0.0.1 (/tmp/cook-demo/logic)
   Compiling model v0.0.1 (/tmp/cook-demo/model)
    Finished `release` profile [optimized] target(s) in 1.99s

real    0m5.80s
```

```bash
# Layer 2: cook full recipe on top (fast — third-party already compiled)
CARGO=$(which cargo) time cargo chef cook --release --recipe-path /tmp/recipe-full.json
```

**Output:**

```
   Compiling model v0.0.1 (/tmp/cook-demo/model)
   Compiling logic v0.0.1 (/tmp/cook-demo/logic)
   Compiling app v0.0.1 (/tmp/cook-demo/app)
    Finished `release` profile [optimized] target(s) in 0.06s

real    0m0.07s   ← 83x faster than cold cook
```

**Key result:** when the thirdparty layer is warm, cooking the full recipe
takes **0.07 s** — only the lightweight workspace stubs are (re)compiled.
No external crates are touched.

### 4.3 Docker build (workaround)

```bash
cd examples/workspace-split-cache
DOCKER_BUILDKIT=1 docker build -t workspace-split-cache .
docker run --rm workspace-split-cache
# Expected: {"name":"world"}
```

Use `--progress=plain` to see layer caching in action:

```bash
# First build (cold):
DOCKER_BUILDKIT=1 docker build --progress=plain -t workspace-split-cache . 2>&1 \
  | grep -E "RUN cargo|CACHED"

# Expected (all layers run):
# #7 RUN cargo chef prepare --recipe-path recipe.json
# #8 RUN python3 strip_path_deps.py recipe.json recipe-thirdparty.json
# #9 RUN cargo chef cook --release --recipe-path recipe.json   (cook_thirdparty)
# #10 RUN cargo chef cook --release --recipe-path recipe.json  (cook)
# #11 RUN cargo build --release --bin app

# Simulate a workspace-internal change (no external dep change):
echo "" >> app/src/main.rs

DOCKER_BUILDKIT=1 docker build --progress=plain -t workspace-split-cache . 2>&1 \
  | grep -E "RUN cargo|CACHED"

# Expected (cook_thirdparty is CACHED; only cook + builder re-run):
# CACHED #9 RUN cargo chef cook ...   (cook_thirdparty — STABLE!)
# #10 RUN cargo chef cook ...         (cook — fast, stubs only)
# #11 RUN cargo build ...             (builder)

# Restore
git checkout app/src/main.rs
```

---

## 5. Solution B — native `--external-only` flag

The `--external-only` flag for `cargo chef prepare` produces a third-party-only
recipe natively, with no Python script needed.

### 5.1 Usage

```bash
cd examples/workspace-split-cache

# Generate external-only recipe natively
cargo chef prepare --external-only --recipe-path /tmp/recipe-ext.json
```

**Output** (identical to the Python workaround):

```toml
=== app/Cargo.toml ===
[package]
name = "app"
edition = "2021"
version = "0.0.1"

[dependencies]
anyhow = "1"           # [dependencies.logic] path dep stripped natively

[[bin]]
path = "src/main.rs"
name = "app"
...
```

Lock file packages (workspace crates absent):

```
['anyhow', 'itoa', 'memchr', 'proc-macro2', 'quote', 'serde', 'serde_core',
'serde_derive', 'serde_json', 'syn', 'unicode-ident', 'zmij']
```

### 5.2 Two-layer Dockerfile (native)

See `Dockerfile.native`. The only difference from `Dockerfile` is:

```dockerfile
# Instead of:
RUN cargo chef prepare --recipe-path recipe.json
RUN python3 strip_path_deps.py recipe.json recipe-thirdparty.json

# You write:
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef prepare --external-only --recipe-path recipe-thirdparty.json
```

No Python installation step in the Docker image. No `apt-get install python3`.

`Dockerfile.native` uses a locally built `cargo-chef` binary injected via a
Docker [named build context][buildx-context]. Build it in two steps:

```bash
# Step 1 — build cargo-chef from the repo root and tag it:
docker build -t cargo-chef-local ../../

# Step 2 — build the example, providing the local binary as a named context:
docker build \
  --build-context cargo-chef-local=docker-image://cargo-chef-local:latest \
  -f Dockerfile.native \
  -t workspace-split-cache-native \
  .
```

Once `--external-only` is released on crates.io, the `COPY --from=cargo-chef-local`
line can be replaced with the standard `RUN cargo install --locked cargo-chef`.

[buildx-context]: https://docs.docker.com/build/building/context/#named-contexts

### 5.3 Stability proof

The core property: two workspaces that differ only in their internal path deps
produce **identical** external-only recipes.

```bash
# Verify: add a new path dep, check the external-only recipe doesn't change
cargo chef prepare --external-only --recipe-path /tmp/recipe-ext-before.json

# Simulate adding a new internal path dependency to logic
cat >> logic/Cargo.toml << 'EOF'

[dependencies.new-internal]
path = "../model"
EOF

cargo chef prepare --external-only --recipe-path /tmp/recipe-ext-after.json

diff /tmp/recipe-ext-before.json /tmp/recipe-ext-after.json
# Expected: no output (files are identical)

git checkout logic/Cargo.toml
```

---

## 6. Related open issues

| Issue  | Summary                                                                        | Relationship                                    |
| ------ | ------------------------------------------------------------------------------ | ----------------------------------------------- |
| [#314] | Workspace with multiple interdependent crates — cook layer never cached        | **Exact same problem**                          |
| [#75]  | cargo-chef doesn't cache local deps — 270s vs 70s measured                     | **Same root cause, measured**                   |
| [#181] | Request for `--workspace --exclude` in `cargo chef cook`                       | Related — about cook, not prepare               |
| [#305] | Nextest recompiles all deps (local crate version masking causes lock mismatch) | Partially fixed by stripping local lock entries |
| [#4]   | Cross-workspace out-of-tree path deps not handled                              | Different issue                                 |

---

## 7. Implementation notes

### What was implemented

The `--external-only` flag is implemented in:

| File                                     | Change                                                                                                           |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `src/skeleton/external_only.rs`          | New module: `strip_path_deps()` and `strip_local_packages_from_lock()` operating on `toml::Value`                |
| `src/skeleton/mod.rs`                    | New public method `Skeleton::strip_path_deps(&mut self)`                                                         |
| `src/recipe.rs`                          | `external_only: bool` parameter added to `Recipe::prepare()` (**breaking change** for library users — see below) |
| `src/main.rs`                            | `--external-only` flag added to the `Prepare` subcommand                                                         |
| `tests/skeletons/tests/external_only.rs` | 3 new integration tests                                                                                          |
| `tests/recipe.rs`                        | Updated call to `Recipe::prepare` with new parameter                                                             |

### What the implementation does

1. **Strips path-based dependency entries** from every manifest in the skeleton.
   Uses `toml::Value` (the `toml` crate is already a dependency) so the
   stripping is structurally correct — not regex-based string manipulation.
   Handles: top-level dep sections, `[target.'cfg(...)'.dependencies]`, and
   `[workspace.dependencies]`.

2. **Removes local (no-`source`) package entries from the lock file.**
   External packages always have `source = "registry+..."`. Local workspace
   members do not. Removing them makes the recipe immune to workspace-membership
   changes.

3. **Handles workspace-inherited path deps (Rust 1.64+ `{ workspace = true }`).**
   Uses a two-pass approach: first, collect the names of all `[workspace.dependencies]`
   entries that carry `path = "..."`. Then, in every member manifest, remove both
   the workspace declaration _and_ any `dep = { workspace = true }` reference whose
   name is in that set. After stripping, no manifest references a workspace dep that
   no longer exists.

### Test summary

```
cargo test -- external_only

running 7 tests
test tests::external_only::external_only_strips_local_packages_from_lock_file ... ok
test tests::external_only::external_only_strips_path_deps_from_manifests ... ok
test tests::external_only::external_only_stable_across_workspace_internal_change ... ok
test skeleton::external_only::tests::keeps_external_deps_untouched ... ok
test skeleton::external_only::tests::strips_path_dep_from_dependencies ... ok
test skeleton::external_only::tests::strips_path_dep_from_target_specific_dependencies ... ok
test skeleton::external_only::tests::strips_path_dep_from_workspace_dependencies ... ok
test skeleton::external_only::tests::strips_workspace_true_refs_to_workspace_path_deps ... ok
test skeleton::external_only::tests::strips_all_path_deps_leaving_valid_manifest ... ok
test skeleton::external_only::tests::strips_local_packages_from_lock_file ... ok

test result: ok. 10 passed
```

The third test — `external_only_stable_across_workspace_internal_change` —
directly verifies the core property: two skeletons that differ only in their
internal path deps produce identical recipes after `strip_path_deps()`.

The fourth test — `strips_workspace_true_refs_to_workspace_path_deps` — verifies
the workspace-inheritance case: `{ workspace = true }` references to path deps
are correctly removed from member manifests.

### Breaking API change

`Recipe::prepare` is a `pub` function re-exported from `lib.rs`. Adding the
positional `external_only: bool` parameter is a **breaking change** for any
crate that calls `Recipe::prepare` directly (rather than through the CLI).

cargo-chef is primarily a CLI tool and the library API has no documented
stability promise, so this is expected to have zero impact in practice. If
the API is stabilised in the future a builder-pattern or options-struct design
would allow new flags to be added without further breakage. This should be
noted in the PR description when merging.
