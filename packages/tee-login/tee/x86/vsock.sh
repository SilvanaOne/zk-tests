#!/bin/bash
echo "Starting vsock-proxy..."
vsock-proxy 8101 dynamodb.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8102 kms.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8103 www.googleapis.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8104 api.github.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
echo "Vsock-proxy started"

