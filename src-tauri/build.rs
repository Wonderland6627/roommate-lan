fn main() {
    // Optional compile-time injection from repo-root .env
    // (ROOMMATE_LOGIN_SERVER / ROOMMATE_AUTH_KEY). Runtime env still wins.
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let env_path = manifest_dir.join("../.env");
    if let Ok(content) = std::fs::read_to_string(&env_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            if key == "ROOMMATE_LOGIN_SERVER" || key == "ROOMMATE_AUTH_KEY" {
                if std::env::var(key).is_err() {
                    // SAFETY: build script only; sets process env for tauri-build child if needed.
                    // Actual embedding uses option_env! from cargo:rustc-env below.
                    println!("cargo:rustc-env={key}={value}");
                }
            }
        }
    }

    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-env-changed=ROOMMATE_LOGIN_SERVER");
    println!("cargo:rerun-if-env-changed=ROOMMATE_AUTH_KEY");

    tauri_build::build()
}
