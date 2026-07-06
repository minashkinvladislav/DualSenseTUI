use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::model::Profile;

pub fn profile_path() -> PathBuf {
    config_root().join("DualSenseTUI").join("profile.json")
}

pub fn load_profile() -> Result<Profile> {
    let path = profile_path();
    if !path.exists() {
        return Ok(Profile::default());
    }

    let data = fs::read_to_string(&path)
        .with_context(|| format!("failed to read profile {}", path.display()))?;
    let mut profile: Profile = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse profile {}", path.display()))?;
    profile.normalize_mappings();
    Ok(profile)
}

pub fn save_profile(profile: &Profile) -> Result<PathBuf> {
    let path = profile_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config dir {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(profile).context("failed to serialize profile")?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write profile {}", path.display()))?;
    Ok(path)
}

fn config_root() -> PathBuf {
    if let Some(path) = non_empty_var("XDG_CONFIG_HOME") {
        return path.into();
    }
    if let Some(home) = non_empty_var("HOME") {
        return Path::new(&home).join(".config");
    }
    PathBuf::from(".")
}

fn non_empty_var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}
