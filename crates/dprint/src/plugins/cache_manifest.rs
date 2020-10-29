use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

use dprint_core::plugins::PluginInfo;
use dprint_core::types::ErrBox;

use crate::environment::Environment;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginCacheManifest {
    schema_version: u16,
    plugins: HashMap<String, PluginCacheManifestItem>,
}

impl PluginCacheManifest {
    pub(super) fn new() -> PluginCacheManifest {
        PluginCacheManifest {
            schema_version: 1,
            plugins: HashMap::new(),
        }
    }

    pub fn add_item(&mut self, key: String, item: PluginCacheManifestItem) {
        self.plugins.insert(key, item);
    }

    pub fn get_item(&self, key: &str) -> Option<&PluginCacheManifestItem> {
        self.plugins.get(key)
    }

    pub fn remove_item(&mut self, key: &str) -> Option<PluginCacheManifestItem> {
        self.plugins.remove(key)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginCacheManifestItem {
    /// Created time in *seconds* since epoch.
    pub created_time: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<u64>,
    pub info: PluginInfo,
}

pub fn read_manifest(environment: &impl Environment) -> PluginCacheManifest {
    return match try_deserialize(environment) {
        Ok(manifest) => {
            if manifest.schema_version != 1 {
                environment.log_error(&format!("Error deserializing plugin cache manifest, but ignoring: Schema version was {}, but expected 1", manifest.schema_version));
                let _ = environment.remove_dir_all(&environment.get_cache_dir().join("plugins"));
                PluginCacheManifest::new()
            } else {
                manifest
            }
        },
        Err(err) => {
            environment.log_error(&format!("Error deserializing plugin cache manifest, but ignoring: {}", err));
            let _ = environment.remove_dir_all(&environment.get_cache_dir());
            PluginCacheManifest::new()
        }
    };

    fn try_deserialize(environment: &impl Environment) -> Result<PluginCacheManifest, ErrBox> {
        let file_path = get_manifest_file_path(environment);
        return match environment.read_file(&file_path) {
            Ok(text) => {
                let manifest = serde_json::from_str::<PluginCacheManifest>(&text)?;
                if manifest.schema_version != 1 {
                    return err!("Schema version was {}, but expected 1", manifest.schema_version);
                } else {
                    Ok(manifest)
                }
            },
            Err(_) => Ok(PluginCacheManifest::new()),
        };
    }
}

pub fn write_manifest(manifest: &PluginCacheManifest, environment: &impl Environment) -> Result<(), ErrBox> {
    let file_path = get_manifest_file_path(environment);
    let serialized_manifest = serde_json::to_string(&manifest)?;
    environment.write_file(&file_path, &serialized_manifest)
}

fn get_manifest_file_path(environment: &impl Environment) -> PathBuf {
    let cache_dir = environment.get_cache_dir();
    cache_dir.join("plugin-cache-manifest.json")
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::environment::TestEnvironment;

    #[test]
    fn it_should_read_ok_manifest() {
        let environment = TestEnvironment::new();
        environment.write_file(
            &environment.get_cache_dir().join("plugin-cache-manifest.json"),
            r#"{
    "schemaVersion": 1,
    "plugins": {
        "a": {
            "createdTime": 123,
            "info": {
                "name": "dprint-plugin-typescript",
                "version": "0.1.0",
                "configKey": "typescript",
                "fileExtensions": [".ts"],
                "helpUrl": "help url",
                "configSchemaUrl": "schema url"
            }
        },
        "c": {
            "createdTime": 456,
            "fileHash": 10,
            "info": {
                "name": "dprint-plugin-json",
                "version": "0.2.0",
                "configKey": "json",
                "fileExtensions": [".json"],
                "helpUrl": "help url 2",
                "configSchemaUrl": "schema url 2"
            }
        }
    }
}"#
        ).unwrap();

        let mut expected_manifest = PluginCacheManifest::new();
        expected_manifest.add_item(String::from("a"), PluginCacheManifestItem {
            created_time: 123,
            file_hash: None,
            info: PluginInfo {
                name: "dprint-plugin-typescript".to_string(),
                version: "0.1.0".to_string(),
                config_key: "typescript".to_string(),
                file_extensions: vec![".ts".to_string()],
                help_url: "help url".to_string(),
                config_schema_url: "schema url".to_string()
            }
        });
        expected_manifest.add_item(String::from("c"), PluginCacheManifestItem {
            created_time: 456,
            file_hash: Some(10),
            info: PluginInfo {
                name: "dprint-plugin-json".to_string(),
                version: "0.2.0".to_string(),
                config_key: "json".to_string(),
                file_extensions: vec![".json".to_string()],
                help_url: "help url 2".to_string(),
                config_schema_url: "schema url 2".to_string()
            }
        });

        assert_eq!(read_manifest(&environment), expected_manifest);
    }

    #[test]
    fn it_should_have_empty_manifest_for_deserialization_error() {
        let environment = TestEnvironment::new();
        environment.write_file(
            &environment.get_cache_dir().join("plugin-cache-manifest.json"),
            r#"{ "plugins": { "a": { file_name: "b", } } }"#
        ).unwrap();

        assert_eq!(read_manifest(&environment), PluginCacheManifest::new());
        assert_eq!(environment.take_logged_errors(), vec![
            String::from("Error deserializing plugin cache manifest, but ignoring: key must be a string at line 1 column 23")
        ]);
    }

    #[test]
    fn it_should_deal_with_non_existent_manifest() {
        let environment = TestEnvironment::new();

        assert_eq!(read_manifest(&environment), PluginCacheManifest::new());
        assert_eq!(environment.take_logged_errors().len(), 0);
    }

    #[test]
    fn it_save_manifest() {
        let environment = TestEnvironment::new();
        let mut manifest = PluginCacheManifest::new();
        manifest.add_item(String::from("a"), PluginCacheManifestItem {
            created_time: 456,
            file_hash: Some(256),
            info: PluginInfo {
                name: "dprint-plugin-typescript".to_string(),
                version: "0.1.0".to_string(),
                config_key: "typescript".to_string(),
                file_extensions: vec![".ts".to_string()],
                help_url: "help url".to_string(),
                config_schema_url: "schema url".to_string()
            }
        });
        manifest.add_item(String::from("b"), PluginCacheManifestItem {
            created_time: 456,
            file_hash: None,
            info: PluginInfo {
                name: "dprint-plugin-json".to_string(),
                version: "0.2.0".to_string(),
                config_key: "json".to_string(),
                file_extensions: vec![".json".to_string()],
                help_url: "help url 2".to_string(),
                config_schema_url: "schema url 2".to_string()
            }
        });
        write_manifest(&manifest, &environment).unwrap();

        // Just read and compare again because the hash map will serialize properties
        // in a non-deterministic order.
        assert_eq!(read_manifest(&environment), manifest);
    }
}
