#!/bin/bash

# Silvana RPC Server Setup Script
# This script sets up NATS JetStream, Nginx with TLS, and gRPC proxy
# Called from user-data.sh after basic system preparation

set -e  # Exit on any error

# Set up logging
exec > >(tee -a /var/log/start-script.log)
exec 2>&1
echo "Starting Silvana RPC setup script at $(date)"

# Configuration
DOMAIN_NAME="rpc-dev.silvana.dev"
EMAIL="dev@silvana.one"
NATS_VERSION="2.11.6"
NATS_CLI_VERSION="0.2.3"

echo "🚀 Initializing Silvana RPC server setup..."

# -------------------------
# Fetch Environment Variables from Parameter Store
# -------------------------
echo "Fetching .env from Parameter Store..."
aws ssm get-parameter \
     --name "/silvana-rpc/dev/env" \
     --with-decryption \
     --query Parameter.Value \
     --output text > /home/ec2-user/zk-tests/packages/avs/rpc/.env

# Lock down permissions
sudo chown ec2-user:ec2-user /home/ec2-user/zk-tests/packages/avs/rpc/.env
sudo chmod 600 /home/ec2-user/zk-tests/packages/avs/rpc/.env

# Update NATS_URL to use the actual domain instead of localhost
echo "Updating NATS_URL to use domain ${DOMAIN_NAME}..."
sudo -u ec2-user sed -i "s|NATS_URL=nats://127.0.0.1:4222|NATS_URL=nats://${DOMAIN_NAME}:4222|g" /home/ec2-user/zk-tests/packages/avs/rpc/.env

echo "✅ Environment variables fetched, updated, and secured"

# -------------------------
# Prepare NATS JetStream Server
# -------------------------
echo "Preparing NATS JetStream server (will install after TLS certificates)..."

# Create nats user
sudo useradd -r -s /bin/false nats 2>/dev/null || echo "nats user already exists"

# Download and install NATS server for ARM64 (Graviton)
echo "Downloading NATS server v${NATS_VERSION}..."
wget -q "https://github.com/nats-io/nats-server/releases/download/v${NATS_VERSION}/nats-server-v${NATS_VERSION}-linux-arm64.tar.gz" -O /tmp/nats-server.tar.gz
cd /tmp
tar -xzf nats-server.tar.gz
sudo mv "nats-server-v${NATS_VERSION}-linux-arm64/nats-server" /usr/local/bin/
sudo chmod +x /usr/local/bin/nats-server
rm -rf /tmp/nats-server*

# Create NATS directories
sudo mkdir -p /etc/nats /var/lib/nats/jetstream /var/log/nats
sudo chown -R nats:nats /var/lib/nats /var/log/nats

echo "✅ NATS server binaries prepared"

# -------------------------
# Nginx and SSL Certificate Setup
# -------------------------
echo "Setting up Nginx and SSL certificates..."

# Install certbot
echo "Installing certbot..."
sudo dnf install -y certbot python3-certbot-nginx

# Verify nginx is installed and create directories
if ! command -v nginx >/dev/null 2>&1; then
    echo "ERROR: Nginx installation failed"
    exit 1
fi

sudo mkdir -p /etc/nginx/conf.d /var/log/nginx /var/cache/nginx

# Start and enable Nginx service
echo "Starting nginx service..."
sudo systemctl start nginx && sudo systemctl enable nginx
sleep 2

# Create nginx user if needed
if ! id nginx >/dev/null 2>&1; then
    echo "Creating nginx user..."
    sudo useradd -r -s /bin/false nginx
fi

# Prepare webroot for ACME challenges
sudo mkdir -p /var/www/letsencrypt/.well-known/acme-challenge
sudo chown -R nginx:nginx /var/www/letsencrypt

# Create initial Nginx configuration for HTTP and ACME challenges
echo "Creating nginx configuration..."
cat <<EOF | sudo tee /etc/nginx/conf.d/rpc-silvana.conf
server {
    listen 80;
    server_name ${DOMAIN_NAME};

    location ^~ /.well-known/acme-challenge/ {
        alias /var/www/letsencrypt/.well-known/acme-challenge/;
    }

    location / {
        return 301 https://\$host\$request_uri;
    }
}
EOF

# Test and reload nginx
echo "Testing nginx configuration..."
if sudo nginx -t; then
    echo "✅ Initial nginx configuration is valid"
    sudo systemctl reload nginx
else
    echo "❌ Initial nginx configuration failed"
    exit 1
fi

# Check for existing certificates in S3 first
echo "Checking for existing SSL certificates in S3..."
if aws s3 cp s3://silvana-tee-images/rpc-cert.tar.gz /tmp/rpc-cert.tar.gz 2>/dev/null; then
    echo "✅ Found existing certificates in S3, extracting..."
    cd /tmp
    sudo tar -xzf rpc-cert.tar.gz -C /
    
    # Verify certificates were extracted successfully
    if sudo test -f "/etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem"; then
        echo "✅ Certificates restored from S3 successfully"
        cert_from_s3=true
    else
        echo "⚠️  Certificate extraction failed, will obtain new certificates"
        cert_from_s3=false
    fi
else
    echo "📋 No existing certificates found in S3, will obtain new ones"
    cert_from_s3=false
fi

# Obtain SSL certificates if not restored from S3
if [ "$cert_from_s3" = false ]; then
    echo "Obtaining new SSL certificates..."
    sudo certbot certonly --webroot -w /var/www/letsencrypt --non-interactive --agree-tos -m "$EMAIL" -d "$DOMAIN_NAME"
    
    # Upload new certificates to S3 for future use
    if sudo test -f "/etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem"; then
        echo "📤 Uploading new certificates to S3..."
        cd /tmp
        sudo tar -czf rpc-cert.tar.gz -C / etc/letsencrypt/live/${DOMAIN_NAME} etc/letsencrypt/archive/${DOMAIN_NAME} etc/letsencrypt/renewal/${DOMAIN_NAME}.conf
        sudo aws s3 cp rpc-cert.tar.gz s3://silvana-tee-images/rpc-cert.tar.gz
        echo "✅ Certificates uploaded to S3 for future deployments"
        sudo rm -f /tmp/rpc-cert.tar.gz
    else
        echo "❌ Certificate generation failed"
        exit 1
    fi
fi

# The gRPC server handles TLS directly on port 443
echo "✅ nginx configured for HTTP only (gRPC uses direct TLS)"

# Test and reload nginx with final configuration
echo "Testing final nginx configuration..."
if sudo nginx -t; then
    echo "✅ Final nginx configuration is valid"
    sudo systemctl reload nginx
    
    # Verify nginx is listening on port 80 (HTTP only)
    sleep 2
    if sudo netstat -tlnp | grep -q ":80.*nginx"; then
        echo "✅ nginx is listening on port 80 (HTTP/redirect only)"
    else
        echo "❌ nginx is NOT listening on port 80"
        echo "Checking nginx error logs..."
        sudo tail -n 10 /var/log/nginx/error.log
        exit 1
    fi
else
    echo "❌ Final nginx configuration failed"
    sudo nginx -t  # Show the error details
    exit 1
fi

# Copy certificates to RPC project directory for easy access
echo "Copying SSL certificates to RPC project directory..."
if sudo test -f "/etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem"; then
    # Create certificates directory in RPC project
    sudo -u ec2-user mkdir -p /home/ec2-user/zk-tests/packages/avs/rpc/certs
    
    # Copy certificates to RPC project directory with proper ownership
    sudo cp "/etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem" /home/ec2-user/zk-tests/packages/avs/rpc/certs/
    sudo cp "/etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem" /home/ec2-user/zk-tests/packages/avs/rpc/certs/
    
    # Set proper ownership and permissions
    sudo chown ec2-user:ec2-user /home/ec2-user/zk-tests/packages/avs/rpc/certs/*
    sudo chmod 600 /home/ec2-user/zk-tests/packages/avs/rpc/certs/*
    
    echo "✅ SSL certificates copied to RPC project directory"
    echo "   📁 Location: /home/ec2-user/zk-tests/packages/avs/rpc/certs/"
    
    # Add certs directory to .gitignore to prevent accidental commit of certificates
    if ! grep -q "^certs/" /home/ec2-user/zk-tests/packages/avs/rpc/.gitignore 2>/dev/null; then
        echo "certs/" | sudo -u ec2-user tee -a /home/ec2-user/zk-tests/packages/avs/rpc/.gitignore > /dev/null
        echo "   🔒 Added certs/ to .gitignore for security"
    fi
else
    echo "❌ SSL certificates not found - gRPC server will run without TLS"
fi

# -------------------------
# Setup SSL Certificate Auto-Renewal
# -------------------------
echo "Setting up automatic SSL renewal..."

cat <<'EOF' | sudo tee /etc/systemd/system/certbot-renew.service
[Unit]
Description=Renew Let\'s Encrypt certificates

[Service]
Type=oneshot
ExecStart=/usr/bin/certbot renew --quiet --deploy-hook "/usr/bin/systemctl reload nginx && /usr/local/bin/upload-renewed-certs.sh && /usr/local/bin/update-rpc-certs.sh"
EOF

cat <<'EOF' | sudo tee /etc/systemd/system/certbot-renew.timer
[Unit]
Description=Run certbot-renew twice daily

[Timer]
OnCalendar=*-*-* 00,12:00:00
RandomizedDelaySec=1h
Persistent=true

[Install]
WantedBy=timers.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now certbot-renew.timer

# Create script to upload renewed certificates to S3
echo "Creating certificate upload script..."
cat <<UPLOAD_SCRIPT | sudo tee /usr/local/bin/upload-renewed-certs.sh
#!/bin/bash
# Script to upload renewed certificates to S3
DOMAIN_NAME="${DOMAIN_NAME}"

echo "\$(date): Uploading renewed certificates to S3..."
cd /tmp
tar -czf rpc-cert-renewed.tar.gz -C / etc/letsencrypt/live/\${DOMAIN_NAME} etc/letsencrypt/archive/\${DOMAIN_NAME} etc/letsencrypt/renewal/\${DOMAIN_NAME}.conf

if aws s3 cp rpc-cert-renewed.tar.gz s3://silvana-tee-images/rpc-cert.tar.gz; then
    echo "\$(date): ✅ Renewed certificates uploaded to S3 successfully"
    rm -f rpc-cert-renewed.tar.gz
else
    echo "\$(date): ❌ Failed to upload renewed certificates to S3"
fi
UPLOAD_SCRIPT

sudo chmod +x /usr/local/bin/upload-renewed-certs.sh

# Create script to update RPC certificates when they get renewed
echo "Creating RPC certificate update script..."
cat <<UPDATE_RPC_SCRIPT | sudo tee /usr/local/bin/update-rpc-certs.sh
#!/bin/bash
# Script to update RPC project certificates after renewal
DOMAIN_NAME="${DOMAIN_NAME}"
RPC_CERTS_DIR="/home/ec2-user/zk-tests/packages/avs/rpc/certs"

echo "\$(date): Updating RPC project certificates..."

if [ -f "/etc/letsencrypt/live/\${DOMAIN_NAME}/fullchain.pem" ]; then
    # Copy renewed certificates to RPC project
    cp "/etc/letsencrypt/live/\${DOMAIN_NAME}/fullchain.pem" "\${RPC_CERTS_DIR}/"
    cp "/etc/letsencrypt/live/\${DOMAIN_NAME}/privkey.pem" "\${RPC_CERTS_DIR}/"
    
    # Set proper ownership and permissions
    chown ec2-user:ec2-user "\${RPC_CERTS_DIR}"/*
    chmod 600 "\${RPC_CERTS_DIR}"/*
    
    echo "\$(date): ✅ RPC project certificates updated successfully"
else
    echo "\$(date): ❌ Failed to find renewed certificates"
fi
UPDATE_RPC_SCRIPT

sudo chmod +x /usr/local/bin/update-rpc-certs.sh

# -------------------------
# Install and Configure NATS JetStream with TLS
# -------------------------
echo "Installing and configuring NATS JetStream server with TLS..."

# Install NATS CLI tool (ARM64 for Graviton)
echo "Installing NATS CLI tool..."
wget -q "https://github.com/nats-io/natscli/releases/download/v${NATS_CLI_VERSION}/nats-${NATS_CLI_VERSION}-arm64.rpm" -O /tmp/nats-cli.rpm
if sudo dnf install -y /tmp/nats-cli.rpm; then
    echo "✅ NATS CLI v${NATS_CLI_VERSION} installed successfully"
    nats --version 2>/dev/null || echo "📋 NATS CLI ready for use"
else
    echo "⚠️  NATS CLI installation failed, continuing without CLI"
fi
rm -f /tmp/nats-cli.rpm

# Setup certificate permissions for NATS
echo "Setting up certificate access for NATS..."
sudo groupadd ssl-cert 2>/dev/null || true
sudo usermod -a -G ssl-cert nats

# Create renewal hooks directory
sudo mkdir -p /etc/letsencrypt/renewal-hooks/deploy

# Create certificate permission script for renewals
cat <<CERT_SCRIPT | sudo tee /etc/letsencrypt/renewal-hooks/deploy/nats-cert-permissions.sh
#!/bin/bash
chgrp ssl-cert /etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem
chgrp ssl-cert /etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem
chmod 640 /etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem
chmod 640 /etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem
systemctl reload-or-restart nats-server
CERT_SCRIPT

sudo chmod +x /etc/letsencrypt/renewal-hooks/deploy/nats-cert-permissions.sh

# Configure and start NATS
if sudo test -f "/etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem"; then
    echo "Setting certificate permissions for NATS..."
    sudo chgrp ssl-cert /etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem
    sudo chgrp ssl-cert /etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem
    sudo chmod 640 /etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem
    sudo chmod 640 /etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem

    echo "Creating NATS configuration with TLS..."
    cat <<EOF | sudo tee /etc/nats/nats-server.conf
# NATS Server Configuration with JetStream and TLS
host: 0.0.0.0
port: 4222

tls {
    cert_file: "/etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem"
    key_file: "/etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem"
    timeout: 5
}

http_port: 8222

websocket {
    host: 0.0.0.0
    port: 8080
    compression: true
    tls {
        cert_file: "/etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem"
        key_file: "/etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem"
    }
}

jetstream {
    store_dir: "/var/lib/nats/jetstream"
    max_memory_store: 1GB
    max_file_store: 10GB
    sync_interval: 2s
}

log_file: "/var/log/nats/nats-server.log"
log_size_limit: 100MB
max_traced_msg_len: 32768
max_payload: 1MB
max_pending: 256MB
max_connections: 64K
write_deadline: "10s"
EOF

    nats_config="TLS enabled"
else
    echo "⚠️  SSL certificates not found, configuring NATS without TLS"
    cat <<EOF | sudo tee /etc/nats/nats-server.conf
# NATS Server Configuration with JetStream (No TLS)
host: 0.0.0.0
port: 4222
http_port: 8222

websocket {
    host: 0.0.0.0
    port: 8080
    compression: true
}

jetstream {
    store_dir: "/var/lib/nats/jetstream"
    max_memory_store: 1GB
    max_file_store: 10GB
    sync_interval: 2s
}

log_file: "/var/log/nats/nats-server.log"
log_size_limit: 100MB
max_traced_msg_len: 32768
max_payload: 1MB
max_pending: 256MB
max_connections: 64K
write_deadline: "10s"
EOF

    nats_config="No TLS (certificates not available)"
fi

# Create NATS systemd service
echo "Creating NATS systemd service..."
cat <<EOF | sudo tee /etc/systemd/system/nats-server.service
[Unit]
Description=NATS JetStream Server
Documentation=https://docs.nats.io/
After=network.target
Wants=network.target

[Service]
Type=simple
User=nats
Group=nats
ExecStart=/usr/local/bin/nats-server -c /etc/nats/nats-server.conf
ExecReload=/bin/kill -s HUP \$MAINPID
KillMode=process
Restart=always
RestartSec=5s
LimitNOFILE=1000000
LimitNPROC=1000000

NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=/var/lib/nats /var/log/nats

[Install]
WantedBy=multi-user.target
EOF

# Start NATS server
echo "Starting NATS JetStream server..."
sudo systemctl daemon-reload
sudo systemctl enable nats-server
sudo systemctl start nats-server

sleep 5

# Verify NATS status
if sudo systemctl is-active --quiet nats-server; then
    echo "✅ NATS JetStream server started successfully (${nats_config})"
    if [ "$nats_config" = "TLS enabled" ]; then
        echo "🔒 NATS (TLS): nats://${DOMAIN_NAME}:4222"
        echo "🔒 NATS-WS (TLS): wss://${DOMAIN_NAME}:8080/ws"
    else
        echo "🔓 NATS: nats://${DOMAIN_NAME}:4222"
        echo "🔓 NATS-WS: ws://${DOMAIN_NAME}:8080/ws"
    fi
    echo "📊 NATS monitoring: http://${DOMAIN_NAME}:8222"
else
    echo "⚠️  NATS server failed to start, checking logs..."
    sudo journalctl -u nats-server -n 10 --no-pager
fi

# -------------------------
# Summary
# -------------------------
echo ""
echo "🎉 Silvana RPC server setup completed at $(date)"
echo ""
echo "📋 Services Status:"
echo "   • Nginx: $(sudo systemctl is-active nginx)"
echo "   • NATS JetStream: $(sudo systemctl is-active nats-server)"
echo "   • SSL Auto-renewal: $(sudo systemctl is-active certbot-renew.timer)"
echo ""
echo "🌐 Endpoints:"
echo "   • gRPC + gRPC-Web (Direct TLS): https://${DOMAIN_NAME}:443"
echo "   • HTTP Redirect: http://${DOMAIN_NAME}:80"
if [ "$nats_config" = "TLS enabled" ]; then
echo "   • NATS (TLS): nats://${DOMAIN_NAME}:4222"
echo "   • NATS-WS (TLS): wss://${DOMAIN_NAME}:8080/ws"
else
echo "   • NATS: nats://${DOMAIN_NAME}:4222"
echo "   • NATS-WS: ws://${DOMAIN_NAME}:8080/ws"
fi
echo "   • NATS Monitoring: http://${DOMAIN_NAME}:8222"
echo "   • Prometheus Metrics: http://${DOMAIN_NAME}:9090/metrics"
echo ""
echo "🔧 Management Commands:"
echo "   • Check NATS status: sudo systemctl status nats-server"
echo "   • Check Nginx status: sudo systemctl status nginx"
echo "   • View NATS logs: sudo journalctl -u nats-server -f"
echo "   • NATS CLI: nats --help"
echo ""

echo "Ready for RPC server deployment! 🚀" 

# cd /home/ec2-user/zk-tests/packages/avs/rpc
# cargo build --release

echo "🔄 Note: After deployment, start the RPC server with:"
echo "   cd /home/ec2-user/zk-tests/packages/avs/rpc"
echo "   cargo build --release"
echo "   sudo setcap CAP_NET_BIND_SERVICE=+eip target/release/rpc"
echo "   cargo run --release"
echo ""
echo "🔒 The gRPC server will automatically detect TLS certificates and enable HTTPS on port 443"
echo "⚡ setcap grants port 443 binding permission without running as root"