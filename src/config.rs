use dirs::home_dir;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    InvalidPath(PathBuf),

    #[error("{0}")]
    DbPathNodExists(PathBuf),

    #[error("failed to get $HOME dir")]
    FaildToGetHome,
}

pub type Result<T> = std::result::Result<T, ConfigError>;

pub struct Config {
    pub dbpath: PathBuf,
}

pub fn create_db_dir_if_not_exists(db_path: &Path) -> Result<()> {
    let dir = db_path
        .parent()
        .ok_or_else(|| ConfigError::InvalidPath(db_path.to_path_buf()))?;

    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }

    Ok(())
}

impl Config {
    fn default_dbpath() -> Result<PathBuf> {
        let mut dir = home_dir().ok_or(ConfigError::FaildToGetHome)?;
        dir.push(".local/share/frecency/db/frecency.db3");
        Ok(dir)
    }

    pub fn new(dbpath: Option<String>) -> Result<Self> {
        let dbpath = if let Some(dbpath_str) = dbpath {
            let mut dbpath = PathBuf::new();
            dbpath.push(dbpath_str);

            if !dbpath.exists() {
                return Err(ConfigError::DbPathNodExists(dbpath));
            }

            let db_path_meta = fs::metadata(&dbpath)?;
            if !db_path_meta.is_file() {
                return Err(ConfigError::InvalidPath(dbpath));
            }
            dbpath
        } else {
            Self::default_dbpath()?
        };

        create_db_dir_if_not_exists(&dbpath)?;

        Ok(Self { dbpath })
    }
}
