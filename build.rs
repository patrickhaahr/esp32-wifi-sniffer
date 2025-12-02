fn main() {
    // Always look for .env next to Cargo.toml so builds work even when run from elsewhere
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set by cargo");
    let env_path = std::path::Path::new(&manifest_dir).join(".env");
    println!("cargo:rerun-if-changed={}", env_path.display());

    // Load .env file if it exists (for local development) so env!() can read WIFI_* at compile time
    if let Err(err) = dotenvy::from_path(&env_path) {
        println!(
            "cargo:warning=No .env file found at {}, using environment variables ({})",
            env_path.display(),
            err
        );
    }

    // Re-export environment variables to make them available to env!() macro
    // This is necessary because build.rs runs in a separate process
    if let Ok(ssid) = std::env::var("WIFI_SSID") {
        println!("cargo:rustc-env=WIFI_SSID={}", ssid);
    }
    if let Ok(pass) = std::env::var("WIFI_PASS") {
        println!("cargo:rustc-env=WIFI_PASS={}", pass);
    }

    embuild::espidf::sysenv::output();
}
