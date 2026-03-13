use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::config_model::EffectiveConfig;

#[allow(dead_code)]
pub fn include_tree(effective: &EffectiveConfig) -> Vec<PathBuf> {
    let mut paths = BTreeSet::new();
    for include in &effective.includes {
        paths.insert(include.source_file.clone());
    }
    paths.into_iter().collect()
}
