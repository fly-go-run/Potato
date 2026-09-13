//! Read-only import of legacy provider and speech settings. No Python process,
//! model call, keychain mutation, or overwrite of configured native settings.
use crate::{required, string, Error, Result, Runtime};
use base64::{engine::general_purpose::URL_SAFE, Engine};
use serde_json::{json, Value};
use std::{fs, io::Read, path::Path};

fn read(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Null);
    }
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::new(400, "Legacy settings must be regular files"));
    }
    let mut data = Vec::new();
    file.take(2_000_001).read_to_end(&mut data)?;
    if data.len() > 2_000_000 {
        return Err(Error::new(413, "Legacy settings file is too large"));
    }
    serde_json::from_slice(&data)
        .map_err(|_| Error::new(400, "Legacy settings contain invalid JSON"))
}
fn cipher(secret: &Path) -> Result<Option<fernet::Fernet>> {
    let path = secret.join(".master_key");
    if !path.exists() {
        return Ok(None);
    }
    let mut text = String::new();
    fs::File::open(path)?.take(129).read_to_string(&mut text)?;
    let text = text.trim();
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::new(400, "Legacy master key is invalid"));
    }
    let bytes: Vec<u8> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect();
    Ok(fernet::Fernet::new(&URL_SAFE.encode(bytes)))
}
fn decrypt(cipher: &Option<fernet::Fernet>, value: &str) -> Result<String> {
    if let Some(token) = value.strip_prefix("ENC:") {
        let cipher=cipher.as_ref().ok_or_else(||Error::new(409,"Open the old application once to export its legacy keychain key, then retry import"))?;
        let bytes = cipher.decrypt(token).map_err(|_| {
            Error::new(
                409,
                "Legacy credential cannot be decrypted; original settings were not changed",
            )
        })?;
        String::from_utf8(bytes).map_err(|_| Error::new(400, "Legacy credential is not text"))
    } else {
        Ok(value.to_owned())
    }
}

impl Runtime {
    pub(crate) fn legacy_settings_status(&self) -> Result<Value> {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(std::path::PathBuf::from);
        let working = std::env::var_os("POTATO_WORKING_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                home.map(|home| {
                    [".potato", ".qwenpaw", ".copaw"]
                        .into_iter()
                        .map(|name| home.join(name))
                        .find(|p| p.join("config.json").is_file() || p.join("workspaces").is_dir())
                        .unwrap_or(home.join(".potato"))
                })
            });
        let working = working
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let secret =
            std::env::var("POTATO_SECRET_DIR").unwrap_or_else(|_| format!("{working}.secret"));
        Ok(
            json!({"working_dir":working,"secret_dir":secret,"last_import":self.db()?.get("legacy_settings_import",Value::Null)?}),
        )
    }
    pub fn import_legacy_settings(&self, working: &Path, secret: &Path) -> Result<Value> {
        if !working.is_absolute() || !secret.is_absolute() {
            return Err(Error::new(400, "Legacy data directories must be absolute"));
        }
        if !working.is_dir() || !secret.is_dir() {
            return Err(Error::new(
                404,
                "Legacy data or secrets directory was not found",
            ));
        }
        let cipher = cipher(secret)?;
        let agent = read(&working.join("workspaces/default/agent.json"))?;
        let active = if agent["active_model"].is_object() {
            agent["active_model"].clone()
        } else {
            read(&secret.join("providers/active_model.json"))?
        };
        let mut imported = Vec::new();
        let mut unsupported = 0;
        for folder in ["builtin", "custom"] {
            let path = secret.join("providers").join(folder);
            if !path.is_dir() {
                continue;
            }
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                if imported.len() > 128 {
                    return Err(Error::new(413, "Too many legacy providers"));
                }
                let mut provider = read(&entry.path())?;
                if !matches!(
                    string(&provider, "chat_model"),
                    "OpenAIChatModel" | "OpenAIResponseModel"
                ) {
                    unsupported += 1;
                    continue;
                }
                if provider["id"] == crate::cloud::PROVIDER { continue; }
                let env_key = self.model_env_key_at(Some(working), string(&provider,"id"), if active["provider_id"] == provider["id"] { string(&active,"model") } else { "" });
                if string(&provider, "api_key").is_empty() && env_key.is_none() {
                    continue;
                }
                required(&provider, "id")?;
                required(&provider, "name")?;
                crate::api::validate_url(required(&provider, "base_url")?)?;
                provider["api_key"] = json!(match env_key { Some(key) => key, None => decrypt(&cipher, string(&provider, "api_key"))? });
                for field in ["models", "extra_models"] {
                    if !provider[field].is_array() {
                        provider[field] = json!([]);
                    }
                }
                imported.push(provider);
            }
        }
        let config = read(&working.join("config.json"))?;
        let image_config = &agent["tools"]["builtin_tools"]["generate_image_gpt"]["config"];
        let mut image_id = None;
        if !string(image_config, "api_key").is_empty() {
            let endpoint = string(image_config, "endpoint");
            let endpoint = if endpoint.trim().is_empty() {
                "https://api.openai.com/v1/images/generations"
            } else {
                endpoint.trim()
            };
            if let Some(base) = endpoint.strip_suffix("/images/generations") {
                crate::api::validate_url(base)?;
                let id = "legacy-image";
                imported.push(json!({"id":id,"name":"图片服务（旧版）","base_url":base,"chat_model":"OpenAIChatModel","api_key":decrypt(&cipher,string(image_config,"api_key"))?,"models":[{"id":"gpt-image-2","name":"GPT Image 2"}],"extra_models":[],"is_custom":true,"require_api_key":true}));
                image_id = Some(id);
            } else {
                unsupported += 1;
            }
        }
        let env = read(&secret.join("envs.json"))?;
        let setting = |names: &[&str]| -> Result<String> {
            for name in names {
                if let Some(value) = env[*name].as_str().filter(|s| !s.is_empty()) {
                    return decrypt(&cipher, value);
                }
            }
            Ok(String::new())
        };
        let speech_key = setting(&[
            "VOLCENGINE_SPEECH_API_KEY",
            "POTATO_SPEECH_API_KEY",
            "apikey",
            "APIKEY",
        ])?;
        let app_id = setting(&[
            "VOLCENGINE_SPEECH_APP_ID",
            "POTATO_SPEECH_APP_ID",
            "keyid",
            "KEYID",
        ])?;
        let mut resource =
            string(&config["agents"], "transcription_doubao_stream_resource_id").to_owned();
        if resource.is_empty() {
            resource = setting(&[
                "POTATO_SPEECH_STREAM_RESOURCE_ID",
                "VOLCENGINE_SPEECH_STREAM_RESOURCE_ID",
            ])?;
        }
        if resource.is_empty() {
            resource = "volc.seedasr.sauc.duration".into();
        }
        let mut db = self.db()?;
        let mut providers = db
            .get("providers", json!([]))?
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut count = 0;
        for mut provider in imported {
            if providers
                .iter()
                .any(|p| p["id"] == provider["id"] && !string(p, "api_key").is_empty())
            {
                continue;
            }
            provider["api_key"] = json!(db.seal(string(&provider, "api_key"))?);
            providers.retain(|p| p["id"] != provider["id"]);
            providers.push(provider);
            count += 1;
        }
        for provider in crate::api::default_providers().as_array().unwrap() {
            if !providers.iter().any(|p| p["id"] == provider["id"]) {
                providers.push(provider.clone());
            }
        }
        let mut values = Vec::new();
        if count > 0 {
            values.push(("providers".into(), json!(providers)));
        }
        let native_active = db.get("active", Value::Null)?;
        if (native_active.is_null() || native_active["provider_id"] == crate::cloud::PROVIDER && db.get("active_manual",Value::Null)? != native_active)
            && active.is_object()
            && providers.iter().any(|p| p["id"] == active["provider_id"])
        {
            values.push(("active".into(), active.clone()));
        }
        let mut voice = false;
        let mut image = false;
        if let Some(id) = image_id {
            if db.get("media", Value::Null)?.is_null() {
                values.push(("media".into(),json!({"speech_provider_id":"","speech_model":"whisper-1","image_provider_id":id,"image_model":"gpt-image-2"})));
                image = true;
            }
        }
        if !speech_key.is_empty() && db.get("doubao", Value::Null)?.is_null() {
            let enabled = config["agents"]["transcription_provider_type"] == "doubao_asr";
            values.push(("doubao".into(),json!({"api_key":db.seal(&speech_key)?,"app_id":app_id,"resource_id":resource,"enabled":enabled})));
            if db.get("speech_type", Value::Null)?.is_null() {
                values.push((
                    "speech_type".into(),
                    json!(if enabled { "doubao_asr" } else { "disabled" }),
                ));
            }
            voice = true;
        }
        let result = json!({"providers_imported":count,"speech_imported":voice,"image_imported":image,"unsupported_providers":unsupported,"original_data_unchanged":true});
        values.push(("legacy_settings_import".into(), result.clone()));
        values.push(("legacy_auto_models".into(), json!(true)));
        values.push(("legacy_auto_models_error".into(), Value::Null));
        db.put_batch(&values)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_legacy_settings_are_reencrypted_and_import_is_repeatable() {
        let tmp = tempfile::tempdir().unwrap();
        let working = tmp.path().join("old");
        let secret = tmp.path().join("old.secret");
        fs::create_dir_all(&working).unwrap();
        fs::create_dir_all(secret.join("providers/custom")).unwrap();
        let key = [42u8; 32];
        let cipher = fernet::Fernet::new(&URL_SAFE.encode(key)).unwrap();
        fs::write(
            secret.join(".master_key"),
            key.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        )
        .unwrap();
        let encrypted = format!("ENC:{}", cipher.encrypt(b"legacy-test-key"));
        let provider = json!({"id":"family","name":"Family","chat_model":"OpenAIResponseModel","api_key":encrypted,"base_url":"https://example.org/v1","models":[{"id":"family-model"}]});
        let path = secret.join("providers/custom/family.json");
        fs::write(&path, provider.to_string()).unwrap();
        fs::write(
            secret.join("providers/active_model.json"),
            json!({"provider_id":"family","model":"family-model"}).to_string(),
        )
        .unwrap();
        fs::write(secret.join("envs.json"),json!({"VOLCENGINE_SPEECH_API_KEY":format!("ENC:{}",cipher.encrypt(b"speech-test-key"))}).to_string()).unwrap();
        fs::write(
            working.join("config.json"),
            json!({"agents":{"transcription_provider_type":"doubao_asr"}}).to_string(),
        )
        .unwrap();
        let runtime = Runtime::open(&tmp.path().join("native")).unwrap();
        fs::create_dir_all(working.join("workspaces/default")).unwrap();
        fs::write(working.join("workspaces/default/agent.json"),json!({"tools":{"builtin_tools":{"generate_image_gpt":{"config":{"api_key":format!("ENC:{}",cipher.encrypt(b"image-test-key")),"endpoint":"https://images.example.org/v1/images/generations"}}}}}).to_string()).unwrap();
        let result = runtime.import_legacy_settings(&working, &secret).unwrap();
        assert_eq!(result["providers_imported"], 2);
        assert_eq!(result["speech_imported"], true);
        assert_eq!(result["image_imported"], true);
        let media = runtime.db().unwrap().get("media", Value::Null).unwrap();
        let image = runtime
            .provider_connection(
                string(&media, "image_provider_id"),
                string(&media, "image_model"),
            )
            .unwrap();
        assert_eq!(image.key, "image-test-key");
        assert!(runtime
            .providers()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "deepseek"));
        assert_eq!(runtime.connection().unwrap().key, "legacy-test-key");
        assert!(!runtime
            .db()
            .unwrap()
            .get("providers", Value::Null)
            .unwrap()
            .to_string()
            .contains("legacy-test-key"));
        assert_eq!(
            runtime.import_legacy_settings(&working, &secret).unwrap()["providers_imported"],
            0
        );
        assert_eq!(read(&path).unwrap(), provider);
        fs::write(secret.join(".master_key"), "00".repeat(32)).unwrap();
        assert!(runtime.import_legacy_settings(&working, &secret).is_err());
        assert_eq!(runtime.connection().unwrap().key, "legacy-test-key");
    }
}
