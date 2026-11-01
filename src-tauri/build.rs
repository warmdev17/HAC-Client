fn main() {
    let path =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../.env");
    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rerun-if-env-changed=HAC_BASE_URL");
    // CI environment takes precedence; never search parent directories for .env.
    let endpoint = match std::env::var("HAC_BASE_URL") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => dotenvy::from_path_iter(path)
            .expect("Create .env from .env.example or set HAC_BASE_URL before building")
            .map(|entry| entry.expect("Invalid .env syntax"))
            .find(|(name, _)| name == "HAC_BASE_URL")
            .map(|(_, value)| value)
            .expect("HAC_BASE_URL is missing from .env"),
        Err(_) => panic!("HAC_BASE_URL must be valid UTF-8"),
    };
    let parsed = url::Url::parse(&endpoint).expect("HAC_BASE_URL must be an absolute HTTPS URL");
    assert!(
        parsed.scheme() == "https"
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none()
            && !endpoint
                .chars()
                .any(|c| c.is_whitespace() || c.is_control()),
        "HAC_BASE_URL must use HTTPS without credentials, query, fragment or whitespace"
    );
    println!(
        "cargo:rustc-env=HAC_BASE_URL={}",
        endpoint.trim_end_matches('/')
    );
    #[cfg(feature = "desktop")]
    tauri_build::build();
}
