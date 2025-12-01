mod wifi;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripherals::Peripherals,
    nvs::EspDefaultNvsPartition,
};

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Starting ESP32 WiFi Sniffer");

    // Initialize hardware peripherals
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Initialize and verify WiFi connection
    let _wifi = wifi::initialize_wifi(peripherals.modem, sys_loop, nvs)?;

    log::info!("WiFi initialized and verified successfully!");
    log::info!("Ready to start sniffing...");

    // Keep the program running
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
