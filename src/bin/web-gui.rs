use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

/// Configuration file structure
#[derive(Debug, Deserialize)]
struct Config {
    server: ServerConfig,
    mqtt: MqttConfig,
    room: RoomConfig,
    stations: Vec<StationConfig>,
    display: DisplayConfig,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct MqttConfig {
    broker: String,
    topic: String,
}

#[derive(Debug, Deserialize)]
struct RoomConfig {
    width: f32,
    height: f32,
}

#[derive(Debug, Deserialize, Clone)]
struct StationConfig {
    id: String,
    x: f32,
    y: f32,
    label: String,
}

#[derive(Debug, Deserialize)]
struct DisplayConfig {
    device_timeout: u64,
    fade_after: u64,
}

/// MQTT device event from ESP32
#[derive(Debug, Deserialize)]
struct MqttDeviceEvent {
    mac: String,
    rssi: i8,
    channel: u8,
    timestamp: u64,
    station: String,
}

/// RSSI reading from a single station
#[derive(Debug, Clone, Serialize)]
struct RssiReading {
    rssi: i8,
    timestamp: u64,
}

/// Device state with readings from all stations
#[derive(Debug, Clone, Serialize)]
struct DeviceState {
    mac: String,
    readings: HashMap<String, RssiReading>,
    last_seen: u64,
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    devices: Arc<RwLock<HashMap<String, DeviceState>>>,
    config: Arc<Config>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Load configuration
    let config_path = Path::new("web/config.toml");
    let config_str = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_str)?;
    
    log::info!("Loaded configuration:");
    log::info!("  Room: {}x{} meters", config.room.width, config.room.height);
    log::info!("  Stations: {}", config.stations.len());
    for station in &config.stations {
        log::info!("    {} at ({}, {})", station.id, station.x, station.y);
    }

    // Create shared state
    let state = AppState {
        devices: Arc::new(RwLock::new(HashMap::new())),
        config: Arc::new(config),
    };

    // Start MQTT subscriber
    let mqtt_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = mqtt_subscriber(mqtt_state).await {
            log::error!("MQTT subscriber error: {:?}", e);
        }
    });

    // Build web server
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(websocket_handler))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    let addr = format!("{}:{}", state.config.server.host, state.config.server.port);
    log::info!("Starting web server on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Serve the main HTML page
async fn index_handler(State(_state): State<AppState>) -> impl IntoResponse {
    let html_path = Path::new("web/index.html");
    
    match fs::read_to_string(html_path) {
        Ok(html) => Html(html),
        Err(e) => {
            log::error!("Failed to read index.html: {:?}", e);
            Html(format!("<html><body><h1>Error loading page: {:?}</h1></body></html>", e))
        }
    }
}

/// WebSocket handler
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}

/// Handle WebSocket connection
async fn websocket_connection(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    
    log::info!("New WebSocket connection");

    // Spawn a task to broadcast device updates
    let tx_task = tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            // Read current device state
            let devices = state.devices.read().await;
            let device_list: Vec<DeviceState> = devices.values().cloned().collect();
            drop(devices);
            
            // Serialize and send
            if let Ok(json) = serde_json::to_string(&device_list) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages (ping/pong, close)
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Close(_) => break,
            _ => {}
        }
    }

    tx_task.abort();
    log::info!("WebSocket connection closed");
}

/// MQTT subscriber task
async fn mqtt_subscriber(state: AppState) -> Result<()> {
    // Parse MQTT broker URL
    let broker_url = state.config.mqtt.broker.clone();
    let broker_url = broker_url.strip_prefix("mqtt://").unwrap_or(&broker_url);
    let parts: Vec<&str> = broker_url.split(':').collect();
    let host = parts[0];
    let port = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(1883);

    log::info!("Connecting to MQTT broker at {}:{}", host, port);

    // Configure MQTT client
    let mut mqtt_options = MqttOptions::new("web-gui", host, port);
    mqtt_options.set_keep_alive(std::time::Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

    // Subscribe to all device topics
    let topic = state.config.mqtt.topic.clone();
    client.subscribe(&topic, QoS::AtMostOnce).await?;
    log::info!("Subscribed to MQTT topic: {}", topic);

    // Process MQTT events
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                // Parse JSON payload
                if let Ok(payload) = std::str::from_utf8(&publish.payload) {
                    if let Ok(event) = serde_json::from_str::<MqttDeviceEvent>(payload) {
                        // Update device state
                        let mut devices = state.devices.write().await;
                        
                        let device = devices.entry(event.mac.clone()).or_insert_with(|| DeviceState {
                            mac: event.mac.clone(),
                            readings: HashMap::new(),
                            last_seen: event.timestamp,
                        });
                        
                        device.readings.insert(
                            event.station.clone(),
                            RssiReading {
                                rssi: event.rssi,
                                timestamp: event.timestamp,
                            },
                        );
                        device.last_seen = event.timestamp;
                        
                        log::debug!(
                            "Device {} seen by {} with RSSI {}",
                            event.mac,
                            event.station,
                            event.rssi
                        );
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("MQTT error: {:?}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
        
        // Periodically clean up old devices
        if rand::random::<u8>() < 10 {
            cleanup_old_devices(&state).await;
        }
    }
}

/// Remove devices that haven't been seen recently
async fn cleanup_old_devices(state: &AppState) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let timeout = state.config.display.device_timeout;
    
    let mut devices = state.devices.write().await;
    devices.retain(|mac, device| {
        let keep = now - device.last_seen < timeout;
        if !keep {
            log::info!("Removing stale device: {}", mac);
        }
        keep
    });
}

// Import for stream operations
use futures_util::{SinkExt, StreamExt};
