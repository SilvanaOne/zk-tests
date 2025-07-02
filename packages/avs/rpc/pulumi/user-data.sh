#!/bin/bash

# Set up logging
exec > >(tee /var/log/user-data.log)
exec 2>&1
echo "Starting user-data script execution at $(date)"

# Update the instance first
echo "Updating the system..."
sudo dnf update -y

# Install required packages (using dnf consistently for Amazon Linux 2023)
# Note: Skip curl since curl-minimal provides the functionality and conflicts with curl package
echo "Installing required packages..."
sudo dnf install -y awscli nano git make gcc protobuf-compiler protobuf-devel --skip-broken

# Try to install nginx, and if it fails, try from a different source
echo "Installing nginx..."
if ! sudo dnf install -y nginx; then
    echo "Standard nginx installation failed, trying alternative approach..."
    # Enable nginx from Amazon Linux extras if available
    if command -v amazon-linux-extras >/dev/null 2>&1; then
        sudo amazon-linux-extras install nginx1 -y
    else
        # For Amazon Linux 2023, try installing from AppStream
        sudo dnf install -y nginx
    fi
fi

# Verify git is installed before proceeding
if ! command -v git >/dev/null 2>&1; then
    echo "Git installation failed, retrying..."
    sudo dnf install -y git-all
fi


# -------------------------
# Install Rust and Cargo
# -------------------------
echo "Installing Rust and Cargo..."
sudo -u ec2-user -i bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
sudo -u ec2-user -i bash -c 'source ~/.cargo/env && rustc --version && cargo --version'

# -------------------------
# Clone zk-tests repository
# -------------------------
echo "Cloning zk-tests repository as ec2-user into /home/ec2-user..."
if command -v git >/dev/null 2>&1; then
    sudo -u ec2-user -i bash -c 'cd /home/ec2-user && git clone --quiet --no-progress --depth 1 https://github.com/SilvanaOne/zk-tests'
else
    echo "ERROR: Git is still not available after installation attempts"
    exit 1
fi

# -------------------------
# Install and Configure NATS JetStream Server
# -------------------------
echo "Installing NATS JetStream server..."

# Create nats user
sudo useradd -r -s /bin/false nats

# Download and install NATS server
NATS_VERSION="2.11.6"
wget -q "https://github.com/nats-io/nats-server/releases/download/v${NATS_VERSION}/nats-server-v${NATS_VERSION}-linux-amd64.tar.gz" -O /tmp/nats-server.tar.gz
cd /tmp
tar -xzf nats-server.tar.gz
sudo mv "nats-server-v${NATS_VERSION}-linux-amd64/nats-server" /usr/local/bin/
sudo chmod +x /usr/local/bin/nats-server
rm -rf /tmp/nats-server*

# Create NATS directories
sudo mkdir -p /etc/nats
sudo mkdir -p /var/lib/nats/jetstream
sudo mkdir -p /var/log/nats
sudo chown -R nats:nats /var/lib/nats
sudo chown -R nats:nats /var/log/nats

# Create initial NATS configuration file (without TLS - will be updated after certificates)
cat <<EOF | sudo tee /etc/nats/nats-server.conf
# NATS Server Configuration with JetStream (Initial - No TLS)

# Network configuration
host: 0.0.0.0
port: 4222

# HTTP monitoring port
http_port: 8222

# WebSocket configuration for NATS-WS (without TLS initially)
websocket {
    # Enable WebSocket on port 8080
    host: 0.0.0.0
    port: 8080
    
    # Enable compression
    compression: true
    
    # Set custom path (default is /ws)
    # path: "/ws"
}

# JetStream configuration
jetstream {
    # Store directory
    store_dir: "/var/lib/nats/jetstream"
    
    # Maximum memory and storage limits
    max_memory_store: 1GB
    max_file_store: 10GB
    
    # Sync options for durability
    sync_interval: 2s
}

# Logging
log_file: "/var/log/nats/nats-server.log"
log_size_limit: 100MB
max_traced_msg_len: 32768

# Limits
max_payload: 1MB
max_pending: 256MB
max_connections: 64K

# Write deadline
write_deadline: "10s"

# Client authentication (optional - can be enabled later)
# accounts {
#   \$SYS {
#     users = [
#       {user: "admin", pass: "password"}
#     ]
#   }
# }
EOF

# Create systemd service file for NATS
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

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=/var/lib/nats /var/log/nats

[Install]
WantedBy=multi-user.target
EOF

# Start NATS service initially without TLS
echo "Starting NATS JetStream server (initially without TLS)..."
sudo systemctl daemon-reload
sudo systemctl enable nats-server
sudo systemctl start nats-server

# Wait a moment for NATS to start
sleep 3

# Verify NATS is running
if sudo systemctl is-active --quiet nats-server; then
    echo "✅ NATS JetStream server started successfully (without TLS)"
    echo "NATS server listening on port 4222"
    echo "NATS WebSocket listening on port 8080"
    echo "NATS monitoring available on port 8222"
    echo "Note: TLS will be configured after SSL certificates are obtained"
else
    echo "WARNING: NATS server failed to start properly"
    echo "Check logs with: sudo journalctl -u nats-server -f"
fi

# Install NATS CLI tool for management
echo "Installing NATS CLI tool..."
NATS_CLI_VERSION="0.1.5"
wget -q "https://github.com/nats-io/natscli/releases/download/v${NATS_CLI_VERSION}/nats-${NATS_CLI_VERSION}-linux-amd64.tar.gz" -O /tmp/nats-cli.tar.gz
cd /tmp
if tar -xzf nats-cli.tar.gz; then
    sudo mv "nats-${NATS_CLI_VERSION}-linux-amd64/nats" /usr/local/bin/
    sudo chmod +x /usr/local/bin/nats
    echo "✅ NATS CLI installed at /usr/local/bin/nats"
else
    echo "⚠️  NATS CLI installation failed, continuing without CLI"
fi
rm -rf /tmp/nats-*

# -------------------------
# Nginx / Certbot setup for gRPC
# -------------------------

# Customize these variables for your domain
DOMAIN_NAME="rpc.silvana.dev"
EMAIL="dev@silvana.one"

# Install certbot - Amazon Linux 2023 has it in the main repos
echo "Installing certbot..."
sudo dnf install -y certbot python3-certbot-nginx

# Verify nginx is installed and create directories
if ! command -v nginx >/dev/null 2>&1; then
    echo "ERROR: Nginx installation failed"
    exit 1
fi

# Create nginx directories if they don't exist
sudo mkdir -p /etc/nginx/conf.d
sudo mkdir -p /var/log/nginx
sudo mkdir -p /var/cache/nginx

# Start and enable Nginx service
echo "Starting nginx service..."
sudo systemctl start nginx && sudo systemctl enable nginx

# Wait a moment for nginx to start
sleep 2

# Verify nginx user exists, if not create it
if ! id nginx >/dev/null 2>&1; then
    echo "Creating nginx user..."
    sudo useradd -r -s /bin/false nginx
fi

# Prepare webroot for ACME challenges
sudo mkdir -p /var/www/letsencrypt/.well-known/acme-challenge
sudo chown -R nginx:nginx /var/www/letsencrypt

# Create initial Nginx configuration for port 80 to serve ACME challenges and redirect everything else to HTTPS
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

# Reload Nginx so that the HTTP server block is active before requesting the certificate
echo "Testing and reloading nginx configuration..."
sudo nginx -t && sudo nginx -s reload

# Obtain SSL certificates with Certbot using the webroot plugin
echo "Obtaining SSL certificates..."
sudo certbot certonly --webroot -w /var/www/letsencrypt --non-interactive --agree-tos -m "$EMAIL" -d "$DOMAIN_NAME"

# Append HTTPS configuration for port 443 to forward to gRPC port 50051
echo "Adding HTTPS configuration..."
cat <<EOF | sudo tee -a /etc/nginx/conf.d/rpc-silvana.conf

# gRPC upstream for load balancing and health checks
upstream grpc_backend {
    server localhost:50051;
}

server {
    listen 443 ssl http2;
    server_name ${DOMAIN_NAME};

    ssl_certificate /etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;

    # Timeout settings (moved to server context)
    client_body_timeout 300;
    client_header_timeout 300;

    # gRPC specific configuration
    location / {
        grpc_pass grpc://grpc_backend;
        grpc_set_header Host \$host;
        grpc_set_header X-Real-IP \$remote_addr;
        grpc_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        grpc_set_header X-Forwarded-Proto \$scheme;
        
        # gRPC specific headers
        grpc_read_timeout 300;
        grpc_send_timeout 300;
        
        # Add CORS headers for gRPC-Web if needed
        add_header 'Access-Control-Allow-Origin' '*' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization,grpc-timeout,grpc-encoding,grpc-accept-encoding' always;
        add_header 'Access-Control-Expose-Headers' 'Content-Length,Content-Range,grpc-status,grpc-message' always;
        
        # Handle preflight requests
        if (\$request_method = 'OPTIONS') {
            add_header 'Access-Control-Allow-Origin' '*';
            add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS';
            add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization,grpc-timeout,grpc-encoding,grpc-accept-encoding';
            add_header 'Access-Control-Max-Age' 1728000;
            add_header 'Content-Type' 'text/plain; charset=utf-8';
            add_header 'Content-Length' 0;
            return 204;
        }
    }
}
EOF

# Reload Nginx to apply the HTTPS configuration
echo "Reloading nginx with final configuration..."
sudo nginx -t && sudo nginx -s reload

# -------------------------------------------------
# Automatic SSL renewal (twice daily) via systemd
# -------------------------------------------------

echo "Setting up automatic SSL renewal..."

# Create a oneshot service that renews certificates and reloads Nginx if anything changed
cat <<'EOF' | sudo tee /etc/systemd/system/certbot-renew.service
[Unit]
Description=Renew Let\'s Encrypt certificates

[Service]
Type=oneshot
ExecStart=/usr/bin/certbot renew --quiet --deploy-hook "/usr/bin/systemctl reload nginx"
EOF

# Create a timer that runs the service every 12 hours
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

# Activate the timer
sudo systemctl daemon-reload
sudo systemctl enable --now certbot-renew.timer

# -------------------------
# Configure NATS with TLS now that certificates are available
# -------------------------
echo "Configuring NATS with TLS now that SSL certificates are available..."

# Give NATS user access to Let's Encrypt certificates
echo "Setting up certificate access for NATS..."
# Add nats user to ssl-cert group (created by certbot)
sudo usermod -a -G ssl-cert nats 2>/dev/null || true

# Create renewal hooks directory if it doesn't exist
sudo mkdir -p /etc/letsencrypt/renewal-hooks/deploy

# Create a script to set proper permissions for certificates after renewal
cat <<'CERT_SCRIPT' | sudo tee /etc/letsencrypt/renewal-hooks/deploy/nats-cert-permissions.sh
#!/bin/bash
# Set permissions for NATS to read SSL certificates
chgrp ssl-cert /etc/letsencrypt/live/rpc.silvana.dev/fullchain.pem
chgrp ssl-cert /etc/letsencrypt/live/rpc.silvana.dev/privkey.pem
chmod 640 /etc/letsencrypt/live/rpc.silvana.dev/fullchain.pem
chmod 640 /etc/letsencrypt/live/rpc.silvana.dev/privkey.pem

# Restart NATS server to reload certificates
systemctl reload-or-restart nats-server
CERT_SCRIPT

sudo chmod +x /etc/letsencrypt/renewal-hooks/deploy/nats-cert-permissions.sh

# Set initial permissions for certificates
if [ -f "/etc/letsencrypt/live/rpc.silvana.dev/fullchain.pem" ]; then
    echo "Setting certificate permissions for NATS..."
    sudo chgrp ssl-cert /etc/letsencrypt/live/rpc.silvana.dev/fullchain.pem
    sudo chgrp ssl-cert /etc/letsencrypt/live/rpc.silvana.dev/privkey.pem
    sudo chmod 640 /etc/letsencrypt/live/rpc.silvana.dev/fullchain.pem
    sudo chmod 640 /etc/letsencrypt/live/rpc.silvana.dev/privkey.pem

    # Update NATS configuration with TLS
    echo "Updating NATS configuration with TLS..."
    cat <<EOF | sudo tee /etc/nats/nats-server.conf
# NATS Server Configuration with JetStream and TLS

# Network configuration
host: 0.0.0.0
port: 4222

# TLS configuration for NATS protocol
tls {
    cert_file: "/etc/letsencrypt/live/rpc.silvana.dev/fullchain.pem"
    key_file: "/etc/letsencrypt/live/rpc.silvana.dev/privkey.pem"
    
    # Optional: Require client certificates
    # verify: true
    # ca_file: "/path/to/ca.pem"
    
    # Timeout for TLS handshake
    timeout: 5
}

# HTTP monitoring port
http_port: 8222

# WebSocket configuration for NATS-WS with TLS
websocket {
    # Enable WebSocket on port 8080
    host: 0.0.0.0
    port: 8080
    
    # Enable compression
    compression: true
    
    # Set custom path (default is /ws)
    # path: "/ws"
    
    # Enable TLS for secure WebSocket (wss://)
    tls {
        cert_file: "/etc/letsencrypt/live/rpc.silvana.dev/fullchain.pem"
        key_file: "/etc/letsencrypt/live/rpc.silvana.dev/privkey.pem"
    }
}

# JetStream configuration
jetstream {
    # Store directory
    store_dir: "/var/lib/nats/jetstream"
    
    # Maximum memory and storage limits
    max_memory_store: 1GB
    max_file_store: 10GB
    
    # Sync options for durability
    sync_interval: 2s
}

# Logging
log_file: "/var/log/nats/nats-server.log"
log_size_limit: 100MB
max_traced_msg_len: 32768

# Limits
max_payload: 1MB
max_pending: 256MB
max_connections: 64K

# Write deadline
write_deadline: "10s"

# Client authentication (optional - can be enabled later)
# accounts {
#   \\\$SYS {
#     users = [
#       {user: "admin", pass: "password"}
#     ]
#   }
# }
EOF

    # Restart NATS server with new TLS configuration
    echo "Restarting NATS server with TLS configuration..."
    sudo systemctl restart nats-server

    # Wait a moment and verify NATS is running with TLS
    sleep 5
    if sudo systemctl is-active --quiet nats-server; then
        echo "✅ NATS JetStream server restarted successfully with TLS"
        echo "🔒 NATS server (TLS) listening on port 4222"
        echo "🔒 NATS WebSocket (TLS) listening on port 8080"
        echo "📊 NATS monitoring available on port 8222"
        echo "🌐 Connect using: nats://rpc.silvana.dev:4222 (TLS required)"
        echo "🌐 WebSocket connect using: wss://rpc.silvana.dev:8080/ws"
    else
        echo "⚠️  NATS server failed to start with TLS, reverting to non-TLS configuration"
        # Revert to the original non-TLS configuration
        cat <<EOF | sudo tee /etc/nats/nats-server.conf
# NATS Server Configuration with JetStream (Fallback - No TLS)

# Network configuration
host: 0.0.0.0
port: 4222

# HTTP monitoring port
http_port: 8222

# WebSocket configuration for NATS-WS (without TLS)
websocket {
    # Enable WebSocket on port 8080
    host: 0.0.0.0
    port: 8080
    
    # Enable compression
    compression: true
    
    # Set custom path (default is /ws)
    # path: "/ws"
}

# JetStream configuration
jetstream {
    # Store directory
    store_dir: "/var/lib/nats/jetstream"
    
    # Maximum memory and storage limits
    max_memory_store: 1GB
    max_file_store: 10GB
    
    # Sync options for durability
    sync_interval: 2s
}

# Logging
log_file: "/var/log/nats/nats-server.log"
log_size_limit: 100MB
max_traced_msg_len: 32768

# Limits
max_payload: 1MB
max_pending: 256MB
max_connections: 64K

# Write deadline
write_deadline: "10s"
EOF
        sudo systemctl restart nats-server
        echo "🔓 NATS server running without TLS on port 4222"
    fi
else
    echo "⚠️  SSL certificates not found, NATS will continue without TLS"
fi

echo "User-data script completed successfully at $(date)"

