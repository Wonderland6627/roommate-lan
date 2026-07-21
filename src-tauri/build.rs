fn main() {
    // Optional compile-time injection from repo-root .env
    // (ROOMMATE_LOGIN_SERVER / ROOMMATE_AUTH_KEY). Runtime env still wins.
    //
    // Public CI releases must NOT embed a long-lived AuthKey (extractable from
    // the binary). Set ROOMMATE_PUBLIC_RELEASE=1 (release.yml does this).
    let public_release = std::env::var("ROOMMATE_PUBLIC_RELEASE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

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
            if key == "ROOMMATE_AUTH_KEY" && public_release {
                println!(
                    "cargo:warning=skipping compile-time ROOMMATE_AUTH_KEY for public release"
                );
                continue;
            }
            if key == "ROOMMATE_LOGIN_SERVER" || key == "ROOMMATE_AUTH_KEY" {
                if std::env::var(key).is_err() {
                    // Actual embedding uses option_env! from cargo:rustc-env below.
                    println!("cargo:rustc-env={key}={value}");
                }
            }
        }
    }

    // Prefer explicit CI env for login server; never embed AuthKey on public release.
    if let Ok(server) = std::env::var("ROOMMATE_LOGIN_SERVER") {
        if !server.is_empty() {
            println!("cargo:rustc-env=ROOMMATE_LOGIN_SERVER={server}");
        }
    }
    if public_release {
        // Ensure AuthKey is not accidentally injected via the process environment.
        println!("cargo:rustc-env=ROOMMATE_AUTH_KEY=");
    } else if let Ok(key) = std::env::var("ROOMMATE_AUTH_KEY") {
        if !key.is_empty() {
            println!("cargo:rustc-env=ROOMMATE_AUTH_KEY={key}");
        }
    }

    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-env-changed=ROOMMATE_LOGIN_SERVER");
    println!("cargo:rerun-if-env-changed=ROOMMATE_AUTH_KEY");
    println!("cargo:rerun-if-env-changed=ROOMMATE_PUBLIC_RELEASE");

    tauri_build::build()
}
