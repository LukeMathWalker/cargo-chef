use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TargetKind {
    Lib { is_proc_macro: bool },
    Bin,
    Test,
    Bench,
    Example,
    BuildScript,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub(crate) path: PathBuf,
    pub(crate) kind: TargetKind,
    pub(crate) name: String,
}
