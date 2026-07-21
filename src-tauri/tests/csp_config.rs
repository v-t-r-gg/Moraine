//! C3: installed desktop must ship an explicit Content-Security-Policy.

#[test]
fn tauri_conf_defines_non_null_csp() {
    let conf = include_str!("../tauri.conf.json");
    let v: serde_json::Value = serde_json::from_str(conf).expect("tauri.conf.json parses");
    let csp = v
        .pointer("/app/security/csp")
        .and_then(|x| x.as_str())
        .expect("app.security.csp must be a string (not null)");
    assert!(
        !csp.is_empty(),
        "CSP must not be empty — C3 beta requires explicit policy"
    );
    assert!(
        csp.contains("default-src"),
        "CSP should constrain default-src: {csp}"
    );
    assert!(
        csp.contains("'self'"),
        "CSP should allow 'self': {csp}"
    );
    // Loopback diagnostics / IPC only — no open https: world load.
    assert!(
        !csp.contains("https:"),
        "CSP should not open all https: origins by default: {csp}"
    );
    assert!(
        csp.contains("127.0.0.1") || csp.contains("localhost"),
        "CSP should allow loopback service discovery: {csp}"
    );
}
