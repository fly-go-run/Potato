//! Compatibility with the user's Potato settings, without loading a project's env.
use crate::{Result, Runtime, string};
use serde_json::{Value, json};
use std::path::Path;

fn key_names(id: &str, model: &str) -> Vec<String> {
    let ident = id
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_ascii_uppercase();
    let mut names = vec![
        format!("{ident}_API_KEY"),
        format!("POTATO_{ident}_API_KEY"),
    ];
    match id.to_ascii_lowercase().as_str() {
        "gemini" => names.push("GOOGLE_API_KEY".into()),
        "kimi" => names.push("MOONSHOT_API_KEY".into()),
        _ => {}
    }
    let model = model.to_ascii_lowercase();
    let openai = ["gpt", "o1", "o3", "o4", "openai"]
        .iter()
        .any(|s| model.contains(s));
    for family in if openai {
        ["OPENAI", "CLAUDE"]
    } else {
        ["CLAUDE", "OPENAI"]
    } {
        names.push(format!("{ident}_{family}"));
    }
    names
}

impl Runtime {
    pub(crate) fn model_env_key_at(
        &self,
        working: Option<&Path>,
        id: &str,
        model: &str,
    ) -> Option<String> {
        if id == crate::cloud::PROVIDER {
            return None;
        }
        // Process environment wins over app env files, then encrypted settings.
        let names = key_names(id, model);
        let valid = |v: String| {
            let v = v.trim();
            (!v.is_empty() && v != "********").then(|| v.to_owned())
        };
        for name in &names {
            if let Some(value) = std::env::var(name).ok().and_then(valid) {
                return Some(value);
            }
        }
        let mut files = vec![self.root.join(".env")];
        if let Some(working) = working {
            files.push(working.join(".env"));
        } else if let Some(parent) = self
            .root
            .parent()
            .filter(|p| p.file_name().is_some_and(|n| n == ".potato"))
        {
            files.push(parent.join(".env"));
        }
        for file in files {
            for name in &names {
                if let Some(value) = crate::search::env_file_key(&file, name).and_then(valid) {
                    return Some(value);
                }
            }
        }
        None
    }

    /// Called by the desktop's normal startup. Explicit QA profiles remain isolated.
    pub fn restore_local_model_settings(&self) -> Result<()> {
        let status = self.legacy_settings_status()?;
        self.restore_local_model_settings_from(
            Path::new(string(&status, "working_dir")),
            Path::new(string(&status, "secret_dir")),
        )
    }

    pub(crate) fn restore_local_model_settings_from(
        &self,
        working: &Path,
        secret: &Path,
    ) -> Result<()> {
        let already = self.db()?.get("legacy_auto_models", json!(false))? == true;
        if !already && working.is_dir() && secret.is_dir() {
            // The importer preserves configured native providers and the selected model.
            if self.import_legacy_settings(working, secret).is_err() {
                self.db()?.put("legacy_auto_models_error", &json!("旧版模型配置未能完整读取，可在设置 → 数据中重新导入；仍可使用云端模型。"))?;
            }
        }
        self.ensure_model_selection()?;
        Ok(())
    }

    pub(crate) fn ensure_model_selection(&self) -> Result<Value> {
        let active = self.db()?.get("active", Value::Null)?;
        let active_valid = self
            .provider_connection(string(&active, "provider_id"), string(&active, "model"))
            .is_ok();
        // Local settings win over automatic cloud fallback, but not a user's explicit choice.
        if active_valid
            && (active["provider_id"] != crate::cloud::PROVIDER
                || self.db()?.get("active_manual", Value::Null)? == active)
        {
            return Ok(active);
        }
        let providers = self.providers()?;
        for provider in providers
            .iter()
            .filter(|p| p["id"] != crate::cloud::PROVIDER)
        {
            for model in ["extra_models", "models"]
                .iter()
                .flat_map(|k| provider[*k].as_array().into_iter().flatten())
            {
                let id = string(provider, "id");
                let model = string(model, "id");
                if self.provider_connection(id, model).is_ok() {
                    let selection = json!({"provider_id":id,"model":model});
                    self.db()?.put("active", &selection)?;
                    return Ok(selection);
                }
            }
        }
        if active_valid {
            return Ok(active);
        }
        let cloud = self.db()?.get("cloud_config", Value::Null)?;
        let model = string(&cloud, "default_model");
        if self.cloud_connection(model).is_ok() {
            let selection = json!({"provider_id":crate::cloud::PROVIDER,"model":model});
            self.db()?.put("active", &selection)?;
            return Ok(selection);
        }
        Ok(active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn legacy(working: &Path, secret: &Path) {
        fs::create_dir_all(working).unwrap();
        fs::create_dir_all(secret.join("providers/custom")).unwrap();
        fs::write(secret.join("providers/custom/compat.json"), json!({"id":"compat-fixture","name":"Compat","base_url":"https://example.test/v1","chat_model":"OpenAIChatModel","api_key":"","models":[{"id":"gpt-fixture"}]}).to_string()).unwrap();
        fs::write(
            secret.join("providers/active_model.json"),
            json!({"provider_id":"compat-fixture","model":"gpt-fixture"}).to_string(),
        )
        .unwrap();
        fs::write(
            working.join(".env"),
            "COMPAT_FIXTURE_OPENAI='fixture-secret'\n",
        )
        .unwrap();
    }

    #[test]
    fn startup_imports_env_only_legacy_provider_once_and_keeps_native_settings() {
        let dir = tempfile::tempdir().unwrap();
        let working = dir.path().join(".potato");
        let secret = dir.path().join(".potato.secret");
        legacy(&working, &secret);
        let root = working.join("native-v1");
        let core = Runtime::open(&root).unwrap();
        core.restore_local_model_settings_from(&working, &secret)
            .unwrap();
        assert_eq!(core.connection().unwrap().key, "fixture-secret");
        assert_eq!(core.connection().unwrap().model, "gpt-fixture");
        assert!(
            !core
                .db()
                .unwrap()
                .get("providers", Value::Null)
                .unwrap()
                .to_string()
                .contains("fixture-secret")
        );
        let old = fs::read(secret.join("providers/custom/compat.json")).unwrap();
        // Changing the app env is picked up without a re-import or restart.
        fs::write(
            working.join(".env"),
            "COMPAT_FIXTURE_OPENAI=updated-fixture\n",
        )
        .unwrap();
        assert_eq!(core.connection().unwrap().key, "updated-fixture");
        drop(core);
        let core = Runtime::open(&root).unwrap();
        core.restore_local_model_settings_from(&working, &secret)
            .unwrap();
        assert_eq!(core.connection().unwrap().key, "updated-fixture");
        assert_eq!(
            fs::read(secret.join("providers/custom/compat.json")).unwrap(),
            old
        );
        // A later startup must not resurrect a provider deliberately removed in native settings.
        core.db().unwrap().put("providers", &json!([])).unwrap();
        core.restore_local_model_settings_from(&working, &secret)
            .unwrap();
        assert!(core.providers().unwrap().is_empty());
    }

    #[test]
    fn configured_native_selection_survives_legacy_import_and_broken_import_does_not_block_startup()
    {
        let dir = tempfile::tempdir().unwrap();
        let working = dir.path().join("old");
        let secret = dir.path().join("old.secret");
        legacy(&working, &secret);
        let core = Runtime::open(&dir.path().join("native")).unwrap();
        let mut provider = crate::api::default_providers()[0].clone();
        provider["api_key"] = json!(core.db().unwrap().seal("native-key").unwrap());
        core.db()
            .unwrap()
            .put("providers", &json!([provider]))
            .unwrap();
        core.db()
            .unwrap()
            .put(
                "active",
                &json!({"provider_id":"deepseek","model":"deepseek-reasoner"}),
            )
            .unwrap();
        core.restore_local_model_settings_from(&working, &secret)
            .unwrap();
        assert_eq!(core.connection().unwrap().key, "native-key");
        assert_eq!(core.connection().unwrap().model, "deepseek-reasoner");
        let broken = Runtime::open(&dir.path().join("broken")).unwrap();
        fs::write(secret.join("providers/custom/compat.json"), "invalid json").unwrap();
        broken
            .restore_local_model_settings_from(&working, &secret)
            .unwrap();
        assert!(broken.cloud_settings().unwrap()["local_config_error"].is_string());
        assert!(broken.ensure_model_selection().unwrap().is_null());
    }

    #[test]
    fn env_compatibility_never_reads_an_arbitrary_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let core = Runtime::open(&dir.path().join("isolated")).unwrap();
        fs::write(
            dir.path().join(".env"),
            "COMPAT_FIXTURE_API_KEY=untrusted\n",
        )
        .unwrap();
        assert!(core.model_env_key_at(None, "compat-fixture", "").is_none());
        fs::write(
            core.root.join(".env"),
            "COMPAT_FIXTURE_API_KEY=app-secret\nPOTATO_CLOUD_API_KEY=must-not-be-used\n",
        )
        .unwrap();
        assert_eq!(
            core.model_env_key_at(None, "compat-fixture", "").as_deref(),
            Some("app-secret")
        );
        assert!(core.model_env_key_at(None, "potato-cloud", "").is_none());
    }
}
