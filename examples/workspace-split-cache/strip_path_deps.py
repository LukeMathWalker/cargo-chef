#!/usr/bin/env python3
"""
Generate a cargo-chef recipe containing only external (third-party) dependencies.

Usage:
    python3 strip_path_deps.py <input-recipe.json> <output-recipe.json>

Why
---
cargo-chef's `recipe.json` contains every workspace manifest including the
intra-workspace path dependencies (`core = { path = "../core" }`).  Because of
this, the Docker layer produced by `cargo chef cook` is invalidated whenever
*any* workspace manifest changes -- even a purely structural change like adding a
new internal crate or splitting an existing one -- even though no external
(third-party) dependency changed at all.

This script produces `recipe-thirdparty.json` by:

  1. Stripping all path-based dependency *sections* from every manifest in the
     recipe.  cargo-chef serialises manifests with `toml::to_string()`, which
     always expands dotted-table dependencies into dedicated `[section.name]`
     headers.  A section whose body contains a `path = "..."` line is an
     intra-workspace dependency; the entire section is removed, leaving only the
     external crates that cargo must download and compile.

  2. Stripping all local (no-source) package entries from the lock file so that
     adding or removing a workspace member does not change the thirdparty recipe
     either.  (cargo-chef already normalises local crate versions to a dummy
     constant, but new members still add new lock entries.)

The resulting recipe is stable across any pure workspace-internal change: the
`cook_thirdparty` Docker layer built from it is only re-run when an actual
third-party dependency is added, removed, or updated.

Implementation notes
--------------------
The script uses structured text parsing rather than a TOML library so that it
works with the Python standard library alone (no third-party packages required).
It relies on the fact that cargo-chef always serialises manifest contents via
`toml::to_string()`, which produces a deterministic, well-structured format:
intra-workspace dependencies always appear as dedicated `[section.dep-name]`
sections, never as inline tables.

Limitations
-----------
* Workspace-inherited deps (`dep = { workspace = true }`) where the shared
  declaration in `[workspace.dependencies]` carries `path = "..."` are not
  handled by this script.  Use the native ``--external-only`` flag instead, which
  handles this case correctly via a two-pass TOML-aware approach.

* Target-conditional dependency sections (`[target.'cfg(...)'.dependencies.*]`)
  are stripped correctly because the section regex matches any header of the form
  `[*.(dependencies|dev-dependencies|build-dependencies).*]`.

Note
----
The native flag ``--external-only`` is now available in cargo-chef:
    cargo chef prepare --external-only --recipe-path recipe-thirdparty.json
This script is kept as a reference for the approach and for older cargo-chef
versions that do not yet include the flag.  For new projects, prefer the native
flag (see Dockerfile.native).
"""

import json
import re
import sys
from typing import List, Optional, Tuple

# ---------------------------------------------------------------------------
# TOML section-level text manipulation
# ---------------------------------------------------------------------------

# Matches any TOML section header line that introduces an intra-workspace path
# dependency entry, e.g.:
#   [dependencies.my-crate]
#   [dev-dependencies.my-crate]
#   [build-dependencies.my-crate]
#   [target.'cfg(unix)'.dependencies.my-crate]
_PATH_DEP_SECTION_RE = re.compile(
    r"^\[(?:[^\]]*\.)?"                     # optional leading table path
    r"(?:dependencies|dev-dependencies|build-dependencies)"
    r"\.[^\]]+\]$"
)

# Matches the start of ANY TOML section or array-of-tables header.
_ANY_SECTION_RE = re.compile(r"^\[")


def _split_into_sections(
    text: str,
) -> List[Tuple[Optional[str], List[str]]]:
    """Split a TOML string into (header, body_lines) pairs.

    The first group collects any lines that appear before the first section
    header; its header is None.
    """
    sections: List[Tuple[Optional[str], List[str]]] = []
    current_header: Optional[str] = None
    current_body: List[str] = []

    for line in text.splitlines(keepends=True):
        stripped = line.rstrip("\n").rstrip()
        if _ANY_SECTION_RE.match(stripped):
            sections.append((current_header, current_body))
            current_header = line
            current_body = []
        else:
            current_body.append(line)

    sections.append((current_header, current_body))
    return sections


def strip_path_deps_from_manifest(contents: str) -> str:
    """Return *contents* (a TOML string) with all path-dep sections removed.

    cargo-chef serialises intra-workspace path dependencies as dedicated
    `[dependencies.NAME]` sections whose body contains `path = "..."`.
    This function removes those sections entirely.
    """
    sections = _split_into_sections(contents)
    result_parts: List[str] = []

    for header, body in sections:
        if header is not None:
            header_stripped = header.rstrip("\n").rstrip()
            if _PATH_DEP_SECTION_RE.match(header_stripped):
                # Only skip if the body actually contains a path assignment.
                has_path = any(
                    line.lstrip().startswith("path =") for line in body
                )
                if has_path:
                    continue  # drop this section
        result_parts.append(header if header is not None else "")
        result_parts.extend(body)

    return "".join(result_parts)


# ---------------------------------------------------------------------------
# Lock-file filtering
# ---------------------------------------------------------------------------


def strip_local_packages_from_lockfile(lock_str: str) -> str:
    """Remove local (no-source) [[package]] entries from a Cargo.lock string.

    External packages always carry a ``source = "registry+..."`` field.
    Local workspace members have no ``source`` field.  Removing them makes the
    thirdparty recipe immune to workspace membership changes (adding/removing
    internal crates).
    """
    sections = _split_into_sections(lock_str)
    result_parts: List[str] = []

    for header, body in sections:
        if header is not None:
            header_stripped = header.rstrip("\n").rstrip()
            if header_stripped == "[[package]]":
                has_source = any(
                    line.lstrip().startswith("source =") for line in body
                )
                if not has_source:
                    continue  # drop local workspace member entry
        result_parts.append(header if header is not None else "")
        result_parts.extend(body)

    return "".join(result_parts)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> None:
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <input.json> <output.json>", file=sys.stderr)
        sys.exit(1)

    input_path, output_path = sys.argv[1], sys.argv[2]

    with open(input_path) as f:
        recipe = json.load(f)

    # Strip path deps from every manifest.
    for manifest in recipe["skeleton"]["manifests"]:
        manifest["contents"] = strip_path_deps_from_manifest(manifest["contents"])

    # Strip local packages from the lock file (if present).
    if recipe["skeleton"].get("lock_file"):
        recipe["skeleton"]["lock_file"] = strip_local_packages_from_lockfile(
            recipe["skeleton"]["lock_file"]
        )

    with open(output_path, "w") as f:
        json.dump(recipe, f, indent=2)
        f.write("\n")


if __name__ == "__main__":
    main()
