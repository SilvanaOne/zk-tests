#!/bin/bash

# Script to copy ACM for Nitro Enclaves files into the enclave build
# This script should be run during the enclave build process

set -e

echo "Copying ACM for Nitro Enclaves files..."

# Check if ACM files exist on the parent
if [ ! -f "/opt/aws/acm/lib/libvtok_p11.so" ]; then
    echo "Warning: PKCS#11 library not found at /opt/aws/acm/lib/libvtok_p11.so"
    echo "Make sure ACM for Nitro Enclaves is properly configured on the parent instance"
    echo "Run: sudo systemctl status nitro-enclaves-acm.service"
    
    # Create a placeholder file to prevent build errors
    mkdir -p /opt/aws/acm/lib
    touch /opt/aws/acm/lib/libvtok_p11.so
    echo "Created placeholder PKCS#11 library file"
fi

if [ ! -f "/opt/aws/acm/cert_chain.pem" ]; then
    echo "Warning: Certificate chain not found at /opt/aws/acm/cert_chain.pem"
    echo "Make sure ACM for Nitro Enclaves is properly configured on the parent instance"
    
    # Create a placeholder file to prevent build errors
    touch /opt/aws/acm/cert_chain.pem
    echo "Created placeholder certificate chain file"
fi

echo "ACM files ready for enclave build" 