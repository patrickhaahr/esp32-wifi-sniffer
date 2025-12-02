fn main() {
<<<<<<< HEAD
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
=======
    // Load .env file if it exists (for local development)
    // This allows env!() macro to access environment variables at compile time
    if let Err(_) = dotenvy::dotenv() {
        println!("cargo:warning=No .env file found, using environment variables");
>>>>>>> 9bf2c24 (feat: enhance MQTT and WiFi integration)
    }

    // Re-export environment variables to make them available to env!() macro
    // This is necessary because build.rs runs in a separate process
    if let Ok(ssid) = std::env::var("WIFI_SSID") {
        println!("cargo:rustc-env=WIFI_SSID={}", ssid);
    }
    if let Ok(pass) = std::env::var("WIFI_PASS") {
        println!("cargo:rustc-env=WIFI_PASS={}", pass);
    }
    if let Ok(broker) = std::env::var("MQTT_BROKER") {
        println!("cargo:rustc-env=MQTT_BROKER={}", broker);
    }
    if let Ok(station) = std::env::var("STATION_ID") {
        println!("cargo:rustc-env=STATION_ID={}", station);
    }

    embuild::espidf::sysenv::output();
}
