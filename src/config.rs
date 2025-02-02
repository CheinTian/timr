use crate::constants::APP_NAME;
use color_eyre::eyre::{eyre, Result};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;
pub struct Config {
    pub log_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Config {
    pub fn init() -> Result<Self> {
        let log_dir = get_default_state_dir()?.join("logs");
        fs::create_dir_all(&log_dir)?;
        let data_dir = get_default_state_dir()?.join("data");
        fs::create_dir_all(&data_dir)?;

        Ok(Self { log_dir, data_dir })
    }
}

pub fn get_project_dir() -> Result<ProjectDirs> {
    let dirs = ProjectDirs::from("", "", APP_NAME)
        .ok_or_else(|| eyre!("Failed to get project directories"))?;

    Ok(dirs)
}

fn get_default_state_dir() -> Result<PathBuf> {
    println!("{:?}",get_project_dir());
    let directory = get_project_dir()?
        .state_dir()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| {
            // 如果获取不到state_dir，使用当前目录下的.local文件夹作为默认值
            PathBuf::from(".local")
        });

    Ok(directory)
}
