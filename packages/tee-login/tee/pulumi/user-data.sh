#!/bin/bash
# Update the instance and install Nitro Enclaves tools, Docker and other utilities
sudo yum update -y
sudo yum install -y aws-nitro-enclaves-cli-devel aws-nitro-enclaves-cli awscli docker nano socat git make nginx certbot python3-certbot-nginx

# Add the current user to the docker group (so you can run docker without sudo)
sudo usermod -aG docker ec2-user
sudo usermod -aG ne ec2-user

# Adjust the enclave allocator memory
ALLOCATOR_YAML=/etc/nitro_enclaves/allocator.yaml
MEM_KEY=memory_mib
DEFAULT_MEM=3072
CPU_KEY=cpu_count
DEFAULT_CPU=1
sudo sed -i "s/^${MEM_KEY}:.*/${MEM_KEY}: ${DEFAULT_MEM}/" "$ALLOCATOR_YAML"
sudo sed -i "s/^${CPU_KEY}:.*/${CPU_KEY}: ${DEFAULT_CPU}/" "$ALLOCATOR_YAML"

# Start and enable Nitro Enclaves allocator and Docker services
sudo systemctl start nitro-enclaves-allocator.service && sudo systemctl enable nitro-enclaves-allocator.service
sudo systemctl start docker && sudo systemctl enable docker
sudo systemctl enable nitro-enclaves-vsock-proxy.service
echo "- {address: dynamodb.us-east-1.amazonaws.com, port: 443}" | sudo tee -a /etc/nitro_enclaves/vsock-proxy.yaml
echo "- {address: kms.us-east-1.amazonaws.com, port: 443}" | sudo tee -a /etc/nitro_enclaves/vsock-proxy.yaml
echo "- {address: www.googleapis.com, port: 443}" | sudo tee -a /etc/nitro_enclaves/vsock-proxy.yaml
echo "- {address: api.github.com, port: 443}" | sudo tee -a /etc/nitro_enclaves/vsock-proxy.yaml


# Restart vsock-proxy processes for various endpoints.
vsock-proxy 8101 dynamodb.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8102 kms.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8103 www.googleapis.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8104 api.github.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &

# -------------------------
# Nginx / Certbot setup
# -------------------------

# Customize these variables for your domain
DOMAIN_NAME="tee2.silvana.dev"
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
cat <<EOF | sudo tee /etc/nginx/conf.d/tee-login.conf
server {
    listen 80;
    server_name ${DOMAIN_NAME};

    location ^~ /.well-known/acme-challenge/ {
        alias /var/www/letsencrypt/.well-known/acme-challenge/;
    }

    location / {
        return 301 https://$host$request_uri;
    }
}
EOF

# Reload Nginx so that the HTTP server block is active before requesting the certificate
sudo nginx -t && sudo nginx -s reload

# Obtain SSL certificates with Certbot using the webroot plugin
sudo certbot certonly --webroot -w /var/www/letsencrypt --non-interactive --agree-tos -m "$EMAIL" -d "$DOMAIN_NAME"

# Append HTTPS reverse-proxy configuration for port 443
cat <<EOF | sudo tee -a /etc/nginx/conf.d/tee-login.conf

server {
    listen 443 ssl;
    server_name ${DOMAIN_NAME};

    ssl_certificate /etc/letsencrypt/live/${DOMAIN_NAME}/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/${DOMAIN_NAME}/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
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

echo "Cloning zk-tests repository as ec2-user into /home/ec2-user..."
sudo -u ec2-user -i bash -c 'git clone --quiet --no-progress --depth 1 https://github.com/SilvanaOne/zk-tests'
sudo -u ec2-user -i bash -c 'cd /home/ec2-user/zk-tests/packages/tee-login/tee/arm && aws s3 cp s3://silvana-tee-images/tee-arm-v2.tar.gz tee-arm-v2.tar.gz --no-progress && tar -xzvf tee-arm-v2.tar.gz'




