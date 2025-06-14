#!/bin/bash
vsock-proxy 8101 dynamodb.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8102 kms.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &

