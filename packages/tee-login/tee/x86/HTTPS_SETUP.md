# HTTPS Setup with AWS Certificate Manager for Nitro Enclaves

This guide explains how to enable HTTPS in your Silvana TEE Login server using AWS Certificate Manager (ACM) for Nitro Enclaves, which allows you to use ACM certificates directly within your enclave via exported certificate and private key files.

## Prerequisites

- AWS EC2 instance with Nitro Enclaves enabled
- ACM certificate created and validated
- Domain name pointing to your EC2 instance

## Your Current Configuration

- **Certificate ARN**: `arn:aws:acm:us-east-1:977098992151:certificate/7097f06e-22ef-464f-a3a6-4585cbebe624`
- **Certificate ID**: `7097f06e-22ef-464f-a3a6-4585cbebe624`
- **Domain**: `tee.silvana.dev` (from Pulumi config)

## Step 1: Set up ACM for Nitro Enclaves on Parent Instance

SSH into your EC2 instance and run the setup script:

```bash
# SSH into your EC2 instance
ssh -i "TEE.pem" ec2-user@your-instance-ip

# Set the certificate ARN
export ACM_CERTIFICATE_ARN="arn:aws:acm:us-east-1:977098992151:certificate/7097f06e-22ef-464f-a3a6-4585cbebe624"

# Run the setup script
chmod +x setup_acm_nitro_enclaves.sh
./setup_acm_nitro_enclaves.sh
```

This script will:

- Install the `aws-nitro-enclaves-acm` package
- Create `/etc/nitro_enclaves/acm.yaml` configuration
- Start the `nitro-enclaves-acm.service`
- Verify the certificate chain and private key files are available

## Step 2: Verify ACM Service is Running

```bash
# Check service status
sudo systemctl status nitro-enclaves-acm.service

# Check logs
sudo journalctl -u nitro-enclaves-acm.service -f

# Verify files exist
ls -la /opt/aws/acm/cert_chain.pem
ls -la /opt/aws/acm/private_key.pem
```

## Step 3: Build and Deploy Updated Enclave

The updated code now includes:

- Direct certificate and private key file loading
- TLS configuration function that loads certificates from ACM files
- Both HTTPS (port 443) and HTTP (port 3000) servers

```bash
# Build the updated enclave
make

# Run the enclave
make run-debug  # for debugging
# or
make run       # for production
```

## Step 4: Test HTTPS Connection

After the enclave is running, test the HTTPS connection:

```bash
# Test HTTPS endpoint
curl -k https://your-domain.com/health_check
curl -k https://tee.silvana.dev/health_check

# Test HTTP endpoint (should still work)
curl http://your-instance-ip:3000/health_check
```

## Environment Variables

Your server expects these environment variables:

```bash
export DB="your-dynamodb-table-name"
export KMS_KEY_ID="your-kms-key-id"
export AWS_REGION="us-east-1"
export AWS_ACCESS_KEY_ID="your-access-key"
export AWS_SECRET_ACCESS_KEY="your-secret-key"
export ACM_CERTIFICATE="7097f06e-22ef-464f-a3a6-4585cbebe624"
```

## How It Works

1. **Parent Instance**: The `nitro-enclaves-acm.service` runs a small helper enclave that fetches your ACM certificate and private key
2. **Certificate Chain**: The certificate chain is written to `/opt/aws/acm/cert_chain.pem`
3. **Private Key**: The private key is written to `/opt/aws/acm/private_key.pem`
4. **Your Enclave**: Your application loads both the certificate chain and private key from these files
5. **Automatic Renewal**: ACM handles certificate renewal, and the helper updates the files automatically

## Troubleshooting

### ACM Service Issues

```bash
# Restart ACM service
sudo systemctl restart nitro-enclaves-acm.service

# Check detailed logs
sudo journalctl -u nitro-enclaves-acm.service --no-pager -l

# Verify configuration
cat /etc/nitro_enclaves/acm.yaml
```

### Certificate Issues

```bash
# Check certificate details
openssl x509 -in /opt/aws/acm/cert_chain.pem -text -noout

# Verify domain matches
openssl x509 -in /opt/aws/acm/cert_chain.pem -noout -subject -ext subjectAltName
```

### Enclave Issues

```bash
# Check enclave logs
sudo nitro-cli describe-enclaves
sudo nitro-cli console --enclave-id <enclave-id>

# Build with debug output
RUST_LOG=debug make run-debug
```

### Network Issues

```bash
# Test local HTTPS
curl -k https://localhost:443/health_check

# Check if port 443 is open
sudo netstat -tlnp | grep :443

# Verify security group allows HTTPS
# (Already configured in your Pulumi setup)
```

## Security Benefits

- **Private key stays within the enclave**: The private key is only accessible within the secure enclave environment
- **Automatic renewal**: ACM handles certificate renewal without manual intervention
- **Attestation**: AWS KMS operations can be restricted to specific enclave measurements
- **No load balancer costs**: Direct HTTPS termination in your enclave
- **Memory-only access**: Private key is loaded into memory and not persisted elsewhere

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ EC2 Parent Instance                                         │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ nitro-enclaves-acm.service                              │ │
│ │ - Fetches ACM certificate                               │ │
│ │ - Writes cert chain to /opt/aws/acm/cert_chain.pem     │ │
│ │ - Writes private key to /opt/aws/acm/private_key.pem   │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Your Nitro Enclave                                      │ │
│ │ - Loads certificate from cert_chain.pem                │ │
│ │ - Loads private key from private_key.pem               │ │
│ │ - Serves HTTPS on port 443                             │ │
│ │ - Serves HTTP on port 3000 (health checks)             │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Next Steps

1. **DNS Configuration**: Make sure `tee.silvana.dev` points to your Elastic IP
2. **Certificate Validation**: Complete DNS validation for your ACM certificate
3. **Monitoring**: Set up CloudWatch monitoring for the ACM service
4. **Testing**: Test certificate renewal behavior
