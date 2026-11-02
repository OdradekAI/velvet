use serde_json::Value;

const MAIN_CAPABILITY: &str = include_str!("../capabilities/default.json");
const RECOVERY_CAPABILITY: &str = include_str!("../capabilities/recovery.json");
const TAURI_CONFIG: &str = include_str!("../tauri.conf.json");

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

pub fn release_security_and_package_policy_is_explicit() -> bool {
    let Ok(value) = serde_json::from_str::<Value>(TAURI_CONFIG) else {
        return false;
    };
    let expected_csp = serde_json::json!({
        "default-src": ["'self'"],
        "connect-src": ["ipc:", "http://ipc.localhost"],
        "img-src": ["'self'", "blob:", "data:"],
        "style-src": ["'self'"],
        "style-src-attr": ["'unsafe-inline'"],
        "script-src": ["'self'"],
        "font-src": ["'self'", "data:"],
        "frame-src": ["'self'", "https:", "tauri-webview:"],
        "object-src": ["'none'"],
        "base-uri": ["'self'"],
        "form-action": ["'self'"]
    });
    value["app"]["security"]["csp"] == expected_csp
        && value["app"]["security"]["dangerousDisableAssetCspModification"] == false
        && value["bundle"]["externalBin"] == serde_json::json!(["binaries/velvet-launcher"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_capability_has_no_broad_core_defaults_or_unrelated_plugins() {
        assert!(main_capability_is_minimal());
        assert!(recovery_capability_is_host_only());
        assert!(release_security_and_package_policy_is_explicit());
        let value: Value = serde_json::from_str(MAIN_CAPABILITY).expect("capability should parse");
        let permissions = value["permissions"]
            .as_array()
            .expect("permissions should be an array");
        assert!(!permissions.iter().any(|permission| {
            permission.as_str().is_some_and(|permission| {
                permission.starts_with("fs:")
                    || permission.starts_with("shell:")
                    || permission.starts_with("opener:")
                    || permission.starts_with("updater:")
            })
        }));
    }
}
