#!/usr/bin/env bash
# Generate self-signed certificates for TLS
# Used by: Mosquitto MQTT broker, Axum web server, ESP32 clients

set -e

# Load SERVER_IP from .env file (used as CN for certificate)
if [ -f ".env" ]; then
  export $(grep "^SERVER_IP" .env | xargs)
else
  echo "Warning: .env file not found. Using default IP."
fi

# Default IP if not set (localhost for development)
SERVER_IP="${SERVER_IP:-127.0.0.1}"

echo "Generating certificates for server IP: $SERVER_IP"

# Create certs directory (shared between mosquitto and web server)
mkdir -p certs
cd certs

# Clean up any existing certificates
rm -f ca.key ca.crt ca.srl server.key server.csr server.crt

echo "=== Generating CA certificate ==="
# Generate CA (we act as our own Certificate Authority)
openssl req -new -x509 -days 3650 -extensions v3_ca \
    -keyout ca.key -out ca.crt -nodes \
    -subj "/CN=ESP-Sniffer-Local-CA"

echo "=== Generating server key ==="
# Generate Server Key
openssl genrsa -out server.key 2048

echo "=== Generating server CSR ==="
# Generate Server Certificate Signing Request
# Use IP as CN and add as SAN (Subject Alternative Name) for modern TLS
openssl req -out server.csr -key server.key -new -nodes \
    -subj "/CN=$SERVER_IP"

echo "=== Signing server certificate ==="
# Create extension file for SAN (required for IP-based certificates)
cat > server_ext.cnf << EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
IP.1 = $SERVER_IP
IP.2 = 127.0.0.1
DNS.1 = localhost
EOF

# Sign the CSR with our CA
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
    -out server.crt -days 3650 -extfile server_ext.cnf

echo "=== Cleanup ==="
# Cleanup temporary files (keep ca.key for future cert generation if needed)
rm -f server.csr server_ext.cnf ca.srl

echo "=== Setting permissions ==="
# Make certs readable by mosquitto docker container (runs as mosquitto user)
chmod 644 server.key ca.key

echo "=== Generating MQTT password file ==="
# Go back to project root
cd ..

# Create mosquitto config directory if it doesn't exist
mkdir -p mosquitto/config

# Load MQTT credentials from .env file or use defaults
if [ -f ".env" ]; then
  export $(grep "^MQTT_USERNAME" .env | xargs 2>/dev/null || true)
  export $(grep "^MQTT_PASSWORD" .env | xargs 2>/dev/null || true)
fi

# Default credentials if not set
MQTT_USERNAME="${MQTT_USERNAME:-elev1}"
MQTT_PASSWORD="${MQTT_PASSWORD:-password}"

echo "Creating MQTT user: $MQTT_USERNAME"

# Create password file (using -c to create new file, -b for password on command line)
mosquitto_passwd -c -b mosquitto/config/passwd "$MQTT_USERNAME" "$MQTT_PASSWORD"

# Set permissions for mosquitto container (secure permissions for password file)
chmod 700 mosquitto/config/passwd

echo ""
echo "=== Certificate generation complete ==="
echo "Files created in ./certs/:"
echo "  ca.crt      - CA certificate (distribute to clients)"
echo "  ca.key      - CA private key (keep secure, for signing new certs)"
echo "  server.crt  - Server certificate (for MQTT broker and web server)"
echo "  server.key  - Server private key (keep secure)"
echo ""
echo "Files created in ./mosquitto/config/:"
echo "  passwd      - MQTT password file with user: $MQTT_USERNAME"
echo ""
echo "Certificate valid for:"
echo "  - IP: $SERVER_IP"
echo "  - IP: 127.0.0.1"
echo "  - DNS: localhost"
echo ""
echo "Next steps:"
echo "  1. Copy ca.crt to ESP32 for client verification"
echo "  2. Update docker-compose.yml to mount ./certs and ./mosquitto/config"
echo "  3. Update mosquitto.conf to use password file"
echo "  4. Restart services"
