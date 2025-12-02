mod mqtt;
mod sniffer;
mod wifi;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripherals::Peripherals,
    nvs::EspDefaultNvsPartition,
};
use std::thread;
use std::time::Duration;

/// Station identifier (from environment)
const STATION_ID: &str = env!("STATION_ID");

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("=== ESP32 WiFi Sniffer ===");
    log::info!("Station ID: {}", STATION_ID);

    // Initialize hardware peripherals
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Connect to WiFi network (needed for MQTT)
    let _wifi = wifi::initialize_wifi_connected(peripherals.modem, sys_loop, nvs)?;

    // Create event channel for sniffer -> MQTT communication
    let (tx, rx) = mqtt::create_event_channel();

    // Set the event sender in the sniffer module
    sniffer::set_event_sender(tx);

    // Start MQTT publisher in a separate thread
    let station_id = STATION_ID.to_string();
    thread::spawn(move || {
        match mqtt::MqttPublisher::new(&station_id, rx) {
            Ok(mut publisher) => {
                if let Err(e) = publisher.run() {
                    log::error!("MQTT publisher error: {:?}", e);
                }
            }
            Err(e) => {
                log::error!("Failed to create MQTT publisher: {:?}", e);
            }
        }
    });

    // Give MQTT a moment to connect
    thread::sleep(Duration::from_secs(1));

    // Start promiscuous mode sniffer (uses AP's channel when connected)
    sniffer::start_sniffer()?;

    log::info!("Sniffer running. Publishing to MQTT...");

    // Main loop - report statistics periodically
    loop {
        thread::sleep(Duration::from_secs(10));
        let count = sniffer::get_packet_count();
        let sent = sniffer::get_sent_count();
        let dropped = sniffer::get_dropped_count();
        log::info!("Packets: {} captured, {} sent to MQTT, {} dropped", count, sent, dropped);
    }
}
