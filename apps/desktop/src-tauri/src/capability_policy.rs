use serde_json::Value;

const MAIN_CAPABILITY: &str = include_str!("../capabilities/default.json");
const RECOVERY_CAPABILITY: &str = include_str!("../capabilities/recovery.json");

pub fn main_capability_is_minimal() -> bool {
    let Ok(value) = serde_json::from_str::<Value>(MAIN_CAPABILITY) else {
        return false;
    };
    let Some(permissions) = value.get("permissions").and_then(Value::as_array) else {
        return false;
    };
    let permissions = permissions
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    !permissions.iter().any(|permission| {
        matches!(
            *permission,
            "core:default"
                | "core:path:default"
                | "core:window:default"
                | "core:tray:default"
                | "core:menu:default"
                | "core:resources:default"
                | "core:image:default"
        )
    }) && permissions.iter().all(|permission| {
        permission.starts_with("core:event:")
            || permission.starts_with("core:webview:")
            || permission.starts_with("dialog:")
            || permission.starts_with("global-shortcut:")
    })
}

pub fn recovery_capability_is_host_only() -> bool {
    let Ok(value) = serde_json::from_str::<Value>(RECOVERY_CAPABILITY) else {
        return false;
    };
    value["windows"] == serde_json::json!(["recovery"])
        && value["permissions"] == serde_json::json!([
            "core:event:allow-listen",
            "core:event:allow-unlisten"
        ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_capability_has_no_broad_core_defaults_or_unrelated_plugins() {
        assert!(main_capability_is_minimal());
        assert!(recovery_capability_is_host_only());
        let value: Value = serde_json::from_str(MAIN_CAPABILITY).expect("capability should parse");
        let permissions = value["permissions"]
            .as_array()
            .expect("permissions should be an array");
        assert!(!permissions.iter().any(|permission| {
            permission.as_str().is_some_and(|permission| {
                permission.starts_with("fs:")
                    || permission.starts_with("shell:")
                    || permission.starts_with("opener:")
            })
        }));
    }
}
