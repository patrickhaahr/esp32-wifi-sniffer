fn main() {
    // Load .env file if it exists (for local development)
    // This allows env!() macro to access WIFI_SSID and WIFI_PASS at compile time
    if let Err(_) = dotenvy::dotenv() {
        println!("cargo:warning=No .env file found, using environment variables");
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
