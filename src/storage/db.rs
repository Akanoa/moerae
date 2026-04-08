use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::storage::schema;

pub struct Database {
    pub conn: Connection,
    pub project_dir: PathBuf,
    pub indexes_dir: PathBuf,
}

impl Database {
    pub fn open(project_id: &str) -> Result<Self, crate::error::InitError> {
        let project_dir = project_dir_for(project_id);
        Self::open_at(project_dir)
    }

    pub fn open_at(project_dir: PathBuf) -> Result<Self, crate::error::InitError> {
        let indexes_dir = project_dir.join("indexes");
        fs::create_dir_all(&indexes_dir)?;

        let db_path = project_dir.join("moerae.db");
        let conn = Connection::open(&db_path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )?;

        schema::create_schema(&conn)?;
        schema::run_migrations(&conn)?;

        Ok(Self {
            conn,
            project_dir,
            indexes_dir,
        })
    }

    pub fn load_config(&self) -> Result<Config, crate::error::InitError> {
        let result: rusqlite::Result<String> = self.conn.query_row(
            "SELECT data FROM config WHERE id = 1",
            [],
            |row| row.get(0),
        );

        match result {
            Ok(json) => {
                let config: Config = serde_json::from_str(&json)
                    .map_err(|e| crate::error::InitError::InvalidConfig(e.to_string()))?;
                Ok(config)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn store_config(&self, config: &Config) -> Result<(), crate::error::InitError> {
        let json = serde_json::to_string(config)
            .map_err(|e| crate::error::InitError::InvalidConfig(e.to_string()))?;

        self.conn.execute(
            "INSERT INTO config (id, data) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET data = excluded.data",
            [&json],
        )?;

        Ok(())
    }

    pub fn index_path(&self, segment_id: i64) -> PathBuf {
        self.indexes_dir.join(format!("{segment_id}.usearch"))
    }

    pub fn index_tmp_path(&self, segment_id: i64) -> PathBuf {
        self.indexes_dir.join(format!("{segment_id}.usearch.tmp"))
    }
}

pub fn project_dir_for(project_id: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(project_id.as_bytes());
    let hash = hasher.finalize();
    let hex = format!("{:x}", hash);
    let short = &hex[..16];

    let base = dirs_base();
    base.join("projects").join(short)
}

fn dirs_base() -> PathBuf {
    if let Ok(home) = std::env::var("MOERAE_HOME") {
        return PathBuf::from(home);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".moerae");
    }
    PathBuf::from(".moerae")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dir_is_deterministic() {
        let a = project_dir_for("test-project");
        let b = project_dir_for("test-project");
        assert_eq!(a, b);
    }

    #[test]
    fn different_projects_get_different_dirs() {
        let a = project_dir_for("project-a");
        let b = project_dir_for("project-b");
        assert_ne!(a, b);
    }

    #[test]
    fn project_dir_is_filesystem_safe() {
        let dir = project_dir_for("../../etc/passwd");
        let name = dir.file_name().unwrap().to_str().unwrap();
        assert!(name.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(name.len(), 16);
    }

    #[test]
    fn open_and_config_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open_at(tmp.path().to_path_buf()).unwrap();

        let config = db.load_config().unwrap();
        assert_eq!(config.segment_capacity, 100);

        let mut custom = Config::default();
        custom.segment_capacity = 42;
        db.store_config(&custom).unwrap();

        let loaded = db.load_config().unwrap();
        assert_eq!(loaded.segment_capacity, 42);

        custom.segment_capacity = 99;
        db.store_config(&custom).unwrap();
        let reloaded = db.load_config().unwrap();
        assert_eq!(reloaded.segment_capacity, 99);
    }
}
