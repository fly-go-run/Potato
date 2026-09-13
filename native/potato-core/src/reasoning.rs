//! Explicit provider/model capabilities; never infer a universal range from a model name.
use serde_json::{Value, json};

pub fn effort_options(provider: &Value, model: &Value) -> Vec<String> {
    let style = model["thinking_param_style"]
        .as_str()
        .or(provider["thinking_param_style"].as_str());
    if style.is_some_and(|s| s != "effort") {
        return vec![];
    }
    let values = model
        .get("reasoning_effort_options")
        .filter(|v| !v.is_null())
        .or_else(|| provider.get("reasoning_effort_options"));
    let mut result = vec![];
    for value in values.and_then(Value::as_array).into_iter().flatten() {
        if let Some(s) = value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
            if !result.iter().any(|v| v == s) {
                result.push(s.to_owned());
            }
        }
    }
    result
}

/// Missing/invalid preferences use the service default (omit the wire parameter).
pub fn effective_effort(provider: &Value, model: &Value) -> Option<String> {
    let value = model["reasoning_effort"]
        .as_str()
        .filter(|s| !s.is_empty())?;
    let known = model
        .get("reasoning_effort_options")
        .is_some_and(|v| !v.is_null())
        || provider
            .get("reasoning_effort_options")
            .is_some_and(|v| !v.is_null());
    if known && !effort_options(provider, model).iter().any(|s| s == value) {
        return None;
    }
    // Preserve manually configured legacy values when capabilities are unknown.
    Some(value.to_owned())
}

pub(crate) fn discovered_model(source: &Value) -> Option<Value> {
    let id = source["id"].as_str()?;
    let mut model = json!({"id":id,"name":source["name"].as_str().unwrap_or(id)});
    for field in [
        "reasoning_effort_options",
        "thinking_param_style",
        "max_tokens",
        "max_input_length",
    ] {
        if let Some(value) = source.get(field) {
            model[field] = value.clone();
        }
    }
    Some(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_options_override_provider_including_explicit_unsupported() {
        let p = json!({"reasoning_effort_options":["low","medium","high"]});
        assert_eq!(
            effort_options(
                &p,
                &json!({"reasoning_effort_options":["high","max","high"]})
            ),
            vec!["high", "max"]
        );
        assert!(effort_options(&p, &json!({"reasoning_effort_options":[]})).is_empty());
        assert!(effort_options(&p, &json!({"thinking_param_style":"budget"})).is_empty());
        assert!(effort_options(&json!({}), &json!({"id":"gpt-example"})).is_empty());
    }
    #[test]
    fn stale_effort_uses_service_default_and_discovery_keeps_capabilities() {
        let model =
            json!({"id":"a","reasoning_effort":"ultra","reasoning_effort_options":["low","high"]});
        assert_eq!(effective_effort(&Value::Null, &model), None);
        let discovered = discovered_model(&model).unwrap();
        assert_eq!(
            effort_options(&Value::Null, &discovered),
            vec!["low", "high"]
        );
        assert!(discovered["reasoning_effort"].is_null());
    }
}
