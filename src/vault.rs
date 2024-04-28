use serde::de::DeserializeOwned;
use serde_json::{from_reader, from_value, Value};
use std::fs::File;
use std::path::PathBuf;

pub struct Vault {
    path: PathBuf,
}

impl Vault {
    pub fn read_vault_values<T: DeserializeOwned>(&self, name: String) -> Result<T, String> {
        let path = self.path.join("config.json");
        let vault_values = File::open(&path)
            .map_err(|e| e.to_string())
            .and_then(|file| from_reader(file).map_err(|e| e.to_string()))
            .and_then(|json: Value| {
                json.get(&name).cloned()
                    .ok_or(format!("Could not find key: {}", &name))
            })
            .and_then(|subconfig| -> Result<T, String> {
                from_value(subconfig).map_err(|e| e.to_string())
            });
        vault_values.map_err(|e| {
            format!(
                "Could not decode key {} in configuration file {}: {}",
                name,
                path.to_str()
                    .unwrap_or("(path cannot be turned into a string)"),
                e
            )
        })
    }
}

#[cfg(test)]
mod tests_read_vault_values {
    use std::fs::File;
    use std::io::Write;

    use serde::Deserialize;
    use tempfile::tempdir;

    use super::Vault;

    #[derive(Deserialize, Eq, PartialEq, Debug)]
    struct TestVaultConfigObject {
        prop_left: String,
        prop_right: u16,
    }

    #[test]
    fn nominal() {
        // Write the config file
        let directory = tempdir().unwrap();
        let config_file_path = directory.path().join("config.json");

        let raw_file = r#"{
"vault_config_object": {
    "prop_left": "bar",
    "prop_right": 15
},
"other_vault_object": {
    "prop_up": true,
    "prop_down": "foo"
}
}"#;
        let mut config_file = File::create(config_file_path).unwrap();
        config_file.write_all(raw_file.as_bytes()).unwrap();

        // Read it
        let vault = Vault{path: directory.path().into()};
        let result: Result<TestVaultConfigObject, String> = vault.read_vault_values("vault_config_object".into());

        assert_eq!(result, Ok(TestVaultConfigObject{
            prop_left: "bar".into(),
            prop_right: 15
        }))
    }
}
