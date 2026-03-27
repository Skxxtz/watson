use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    utils::errors::{WatsonError, WatsonErrorKind},
    watson_err,
};

pub fn expand_path<T: AsRef<Path>>(path: T, home: &Path) -> PathBuf {
    let path = path.as_ref();
    let mut components = path.components();
    if let Some(std::path::Component::Normal(first)) = components.next() {
        if first == "~" {
            return home.join(components.as_path());
        }
    }
    path.to_path_buf()
}
pub fn home_dir() -> Result<PathBuf, WatsonError> {
    std::env::var("HOME")
        .map_err(|e| watson_err!(WatsonErrorKind::EnvVar, e.to_string()))
        .map(PathBuf::from)
}

fn get_xdg_dirs() -> xdg::BaseDirectories {
    xdg::BaseDirectories::with_prefix("watson")
}

fn legacy_path() -> Result<PathBuf, WatsonError> {
    let home_dir = home_dir()?;
    Ok(home_dir.join(".watson"))
}

/// Returns the configuration directory.
///
/// It first checks for the legacy `~/.watson` directory. If it exists, it returns that path.
/// Otherwise, it returns the XDG standard configuration path, `$XDG_CONFIG_HOME/watson`.
/// If the directory does not exist, it will be created.
pub fn get_config_dir() -> Result<PathBuf, WatsonError> {
    let xdg_dirs = get_xdg_dirs();
    let dir = xdg_dirs
        .get_config_home()
        .ok_or_else(|| watson_err!(WatsonErrorKind::DirRead, "Could not find config directory"))?;
    fs::create_dir_all(&dir).map_err(|e| watson_err!(WatsonErrorKind::DirCreate, e.to_string()))?;
    Ok(dir)
}

/// Returns the data directory.
///
/// It first checks for the legacy `~/.watson` directory. If it exists, it returns that path.
/// Otherwise, it returns the XDG standard data path, `$XDG_DATA_HOME/watson`.
/// If the directory does not exist, it will be created.
pub fn get_data_dir() -> Result<PathBuf, WatsonError> {
    let legacy_path = legacy_path()?;
    if legacy_path.exists() {
        return Ok(legacy_path);
    }
    let xdg_dirs = get_xdg_dirs();
    let dir = xdg_dirs
        .get_data_home()
        .ok_or_else(|| watson_err!(WatsonErrorKind::DirRead, "Could not find data directory"))?;
    fs::create_dir_all(&dir).map_err(|_| {
        watson_err!(
            WatsonErrorKind::DirCreate,
            "Could not create data directory"
        )
    })?;
    Ok(dir)
}

/// Returns the cache directory.
///
/// This function returns the XDG standard cache path, `$XDG_CACHE_HOME/watson`.
/// If the directory does not exist, it will be created.
pub fn get_cache_dir() -> Result<PathBuf, WatsonError> {
    let xdg_dirs = get_xdg_dirs();
    let dir = xdg_dirs
        .get_cache_home()
        .ok_or_else(|| watson_err!(WatsonErrorKind::DirRead, "Could not find cache directory"))?;
    fs::create_dir_all(&dir).map_err(|e| watson_err!(WatsonErrorKind::DirCreate, e.to_string()))?;
    Ok(dir)
}
