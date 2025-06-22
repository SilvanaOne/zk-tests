#!/bin/bash

# Setup script for AWS Certificate Manager for Nitro Enclaves
# This script must be run on the parent EC2 instance

set -e

echo "Setting up AWS Certificate Manager for Nitro Enclaves..."

# Check if running on Amazon Linux 2023
if ! grep -q "Amazon Linux" /etc/os-release; then
    echo "Warning: This script is designed for Amazon Linux 2023"
fi

# Install ACM for Nitro Enclaves
echo "Installing aws-nitro-enclaves-acm package..."
sudo yum update -y
sudo yum install -y aws-nitro-enclaves-acm

# Get certificate ARN from environment or parameter
if [ -z "$ACM_CERTIFICATE_ARN" ]; then
    echo "Error: ACM_CERTIFICATE_ARN environment variable is not set"
    echo "Please set it with: export ACM_CERTIFICATE_ARN='arn:aws:acm:us-east-1:977098992151:certificate/d4078b9f-164d-49b8-963f-f3afd011fbb6'"
    exit 1
fi

echo "Using ACM Certificate ARN: $ACM_CERTIFICATE_ARN"

# Create ACM configuration
echo "Creating ACM configuration..."
sudo mkdir -p /etc/nitro_enclaves

# Create acm.yaml configuration file
sudo tee /etc/nitro_enclaves/acm.yaml > /dev/null <<EOF
---
tokens:
  - label: silvana-tee-token
    source:
      Acm:
        certificate_arn: "$ACM_CERTIFICATE_ARN"
    refresh_interval_secs: 43200
enclave:
  cpu_count: 2
  memory_mib: 256
EOF

echo "ACM configuration created at /etc/nitro_enclaves/acm.yaml"

# Enable and start the ACM service
echo "Enabling and starting nitro-enclaves-acm service..."
sudo systemctl enable nitro-enclaves-acm.service
sudo systemctl start nitro-enclaves-acm.service

# Check service status
echo "Checking service status..."
sudo systemctl status nitro-enclaves-acm.service --no-pager -l

# Wait a moment for the service to initialize
echo "Waiting for ACM service to initialize..."
sleep 10

# Verify certificate chain file exists
if [ -f "/opt/aws/acm/cert_chain.pem" ]; then
    echo "✓ Certificate chain found at /opt/aws/acm/cert_chain.pem"
    echo "Certificate details:"
    openssl x509 -in /opt/aws/acm/cert_chain.pem -text -noout | head -20
else
    echo "✗ Certificate chain file not found. Check service logs."
    sudo journalctl -u nitro-enclaves-acm.service --no-pager -l
    exit 1
fi

# Verify private key file exists
if [ -f "/opt/aws/acm/private_key.pem" ]; then
    echo "✓ Private key found at /opt/aws/acm/private_key.pem"
    echo "Private key details:"
    openssl pkey -in /opt/aws/acm/private_key.pem -text -noout | head -10
else
    echo "✗ Private key file not found. Check service logs."
    sudo journalctl -u nitro-enclaves-acm.service --no-pager -l
    exit 1
fi

echo ""
echo "✓ ACM for Nitro Enclaves setup completed successfully!"
echo ""
echo "Next steps:"
echo "1. Make sure your Nitro Enclave has access to:"
echo "   - /opt/aws/acm/cert_chain.pem (certificate chain)"
echo "   - /opt/aws/acm/private_key.pem (private key)"
echo "2. Update your security group to allow HTTPS traffic on port 443"
echo "3. Start your enclave with the updated configuration"
echo ""
echo "To check service logs: sudo journalctl -u nitro-enclaves-acm.service -f"
echo "To restart service: sudo systemctl restart nitro-enclaves-acm.service" 