use crate::goals::PeriodsConfiguration;
use std::path::PathBuf;
use std::fs::{File};
use serde::Deserialize;
use serde_json::from_reader;
use std::any::Any;

pub struct Vault {
    path: PathBuf
}

impl Vault {
    pub fn read_vault_values<'a, T: Deserialize<'a>>(&self, name: String) -> Result<T, String>{
        let path = self.path.join("config.json");
        let config = File::open(path).and_then(from_reader).map(|x| x[name]);
    }
}

