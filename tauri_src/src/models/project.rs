use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::projects;
use crate::utils::hash_id;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub user: String,
    pub port: u16,
    pub key_path: Option<String>,
    pub remote_path: String,
}

impl SshConfig {
    /// Canonical connection string used as the ControlMaster key and socket path input.
    pub fn connection_key(&self) -> String {
        format!("{}@{}:{}", self.user, self.host, self.port)
    }
}

#[derive(Queryable, Selectable, Serialize, Debug)]
#[diesel(table_name = projects)]
pub struct Project {
    pub id: String,
    pub path: String,
    pub name: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_opened: Option<NaiveDateTime>,
    pub connection_type: String,
    pub ssh_config: Option<String>,
}

impl Project {
    /// Parse the stored JSON ssh_config column into an SshConfig struct.
    pub fn parse_ssh_config(&self) -> Option<SshConfig> {
        self.ssh_config
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
    }

    pub fn is_ssh(&self) -> bool {
        self.connection_type == "ssh"
    }
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = projects)]
pub struct NewProject {
    pub id: String,
    pub path: String,
    pub name: Option<String>,
    pub last_opened: Option<NaiveDateTime>,
    pub connection_type: String,
    pub ssh_config: Option<String>,
}

impl NewProject {
    pub fn new(path: String, name: Option<String>) -> Self {
        let now = chrono::Utc::now().naive_utc();
        let id = hash_id::project_id(&path);
        Self {
            id,
            path,
            name,
            last_opened: Some(now),
            connection_type: "local".to_string(),
            ssh_config: None,
        }
    }

    pub fn new_ssh(config: &SshConfig) -> Self {
        let canonical = format!("{}@{}:{}", config.user, config.host, config.remote_path);
        let now = chrono::Utc::now().naive_utc();
        let id = hash_id::project_id(&canonical);
        let name = config
            .remote_path
            .split('/')
            .filter(|s| !s.is_empty())
            .last()
            .map(str::to_string);
        Self {
            id,
            path: canonical,
            name,
            last_opened: Some(now),
            connection_type: "ssh".to_string(),
            ssh_config: Some(serde_json::to_string(config).unwrap()),
        }
    }
}

#[derive(AsChangeset, Debug)]
#[diesel(table_name = projects)]
pub struct ProjectLastOpenedUpdate {
    pub last_opened: Option<NaiveDateTime>,
}
