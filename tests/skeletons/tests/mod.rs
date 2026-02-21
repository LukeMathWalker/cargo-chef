use std::path::Path;

use crate::support::{check, CargoWorkspace};
use assert_fs::prelude::*;
use assert_fs::TempDir;
use chef::Skeleton;
use expect_test::expect;
use predicates::prelude::*;

mod core;
mod masking;
mod toolchain;
mod workspace;
