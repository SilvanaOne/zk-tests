#!/bin/bash
# Update the instance and install required packages
sudo yum update -y
sudo yum install -y awscli docker nano git make nginx certbot python3-certbot-nginx curl gcc

# Add the current user to the docker group (so you can run docker without sudo)
sudo usermod -aG docker ec2-user

# Start and enable Docker service
sudo systemctl start docker && sudo systemctl enable docker

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
sudo -u ec2-user -i bash -c 'git clone --quiet --no-progress --depth 1 https://github.com/SilvanaOne/zk-tests'

# -------------------------
# Nginx / Certbot setup for gRPC
# -------------------------

# Customize these variables for your domain
DOMAIN_NAME="rpc.silvana.dev"
EMAIL="dev@silvana.one"

# Enable EPEL repository (provides certbot on Amazon Linux)
if command -v amazon-linux-extras >/dev/null 2>&1; then
  # Amazon Linux 2
  sudo amazon-linux-extras install epel -y
else
  # Amazon Linux 2023 uses dnf
  sudo dnf install -y epel-release
fi

# Ensure Certbot and its Nginx plugin are installed (may have failed before EPEL enabled)
sudo dnf install -y certbot python3-certbot-nginx || sudo yum install -y certbot python3-certbot-nginx

# Start and enable Nginx service
sudo systemctl start nginx && sudo systemctl enable nginx

# Prepare webroot for ACME challenges
sudo mkdir -p /var/www/letsencrypt/.well-known/acme-challenge
sudo chown -R nginx:nginx /var/www/letsencrypt

# Create initial Nginx configuration for port 80 to serve ACME challenges and redirect everything else to HTTPS
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
sudo nginx -t && sudo nginx -s reload

# Obtain SSL certificates with Certbot using the webroot plugin
sudo certbot certonly --webroot -w /var/www/letsencrypt --non-interactive --agree-tos -m "$EMAIL" -d "$DOMAIN_NAME"

# Append HTTPS configuration for port 443 to forward to gRPC port 50051
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
        client_body_timeout 300;
        client_header_timeout 300;
        
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
sudo nginx -t && sudo nginx -s reload

# -------------------------------------------------
# Automatic SSL renewal (twice daily) via systemd
# -------------------------------------------------

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

