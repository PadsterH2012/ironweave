use serde::Deserialize;
use std::path::PathBuf;

use crate::auth::AuthConfig;

#[derive(Debug, Deserialize, Clone)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    pub master_key: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct FilesystemConfig {
    pub browse_roots: Vec<String>,
    pub mount_base: String,
    pub idle_unmount_minutes: Option<u64>,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            browse_roots: vec!["/home/paddy".to_string()],
            mount_base: "/home/paddy/ironweave/mounts".to_string(),
            idle_unmount_minutes: Some(30),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BuildServerConfig {
    /// SSH destination for the build server (e.g. "paddy@10.202.28.171")
    pub ssh_target: String,
    /// Path to the source directory on the build server
    pub remote_source_dir: String,
    /// Local source directory to rsync from (defaults to project directory)
    pub local_source_dir: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_path: PathBuf,
    pub data_dir: PathBuf,
    pub tls: Option<TlsConfig>,
    pub auth: Option<AuthConfig>,
    pub security: Option<SecurityConfig>,
    pub filesystem: Option<FilesystemConfig>,
    pub build_server: Option<BuildServerConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
            database_path: PathBuf::from("data/ironweave.db"),
            data_dir: PathBuf::from("data"),
            tls: None,
            auth: None,
            security: None,
            filesystem: None,
            build_server: None,
        }
    }
}
