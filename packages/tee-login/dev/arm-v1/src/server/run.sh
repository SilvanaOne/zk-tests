#!/bin/sh
# Copyright (c), Mysten Labs, Inc.
# SPDX-License-Identifier: Apache-2.0

# - Setup script for server that acts as an init script
# - Sets up Python and library paths
# - Configures loopback network and /etc/hosts
# - Waits for secrets.json to be passed from the parent instance. 
# - Forwards VSOCK port 3000 to localhost:3000
# - Optionally pulls secrets and sets in environmen variables.
# - Launches server

set -e # Exit immediately if a command exits with a non-zero status
echo "run.sh script is running"
export PYTHONPATH=/lib/python3.11:/usr/local/lib/python3.11/lib-dynload:/usr/local/lib/python3.11/site-packages:/lib
export LD_LIBRARY_PATH=/lib:$LD_LIBRARY_PATH

echo "Script completed."
# Assign an IP address to local loopback
busybox ip addr add 127.0.0.1/32 dev lo
busybox ip link set dev lo up

# Add a hosts record, pointing target site calls to local loopback
echo "127.0.0.1   localhost" > /etc/hosts
echo "127.0.0.64   dynamodb.us-east-1.amazonaws.com" >> /etc/hosts
echo "127.0.0.65   kms.us-east-1.amazonaws.com" >> /etc/hosts
echo "127.0.0.66   www.googleapis.com" >> /etc/hosts
echo "127.0.0.67   api.github.com" >> /etc/hosts

# == ATTENTION: code should be generated here that parses allowed_endpoints.yaml and populate domains here ===

cat /etc/hosts

# Get a json blob with key/value pair for secrets
echo "Getting Configuration"
JSON_RESPONSE=$(socat - VSOCK-LISTEN:7777,reuseaddr)
echo "Configuration received:"
echo "$JSON_RESPONSE"
# Sets all key value pairs as env variables that will be referred by the server
# This is shown as a example below. For production usecases, it's best to set the
# keys explicitly rather than dynamically.
echo "$JSON_RESPONSE" | jq -r 'to_entries[] | "\(.key)=\(.value)"' > /tmp/kvpairs ; while IFS="=" read -r key value; do export "$key"="$value"; done < /tmp/kvpairs ; rm -f /tmp/kvpairs

# Run traffic forwarder in background and start the server
# Forwards traffic from 127.0.0.x -> Port 443 at CID 3 Listening on port 800x
# There is a vsock-proxy that listens for this and forwards to the respective domains

# == ATTENTION: code should be generated here that added all hosts to forward traffic ===
# Traffic-forwarder-block
# python3 /traffic_forwarder.py 127.0.0.64 443 3 8101 &
# python3 /traffic_forwarder.py 127.0.0.65 443 3 8102 &
# python3 /traffic_forwarder.py 127.0.0.66 443 3 8103 &
# python3 /traffic_forwarder.py 127.0.0.67 443 3 8104 &

/forwarder 127.0.0.64 443 3 8101 &
/forwarder 127.0.0.65 443 3 8102 &
/forwarder 127.0.0.66 443 3 8103 &
/forwarder 127.0.0.67 443 3 8104 &

# Listens on Local VSOCK Port 3000 and forwards to localhost 3000
socat VSOCK-LISTEN:3000,reuseaddr,fork TCP:localhost:3000 &
echo "Traffic forwarder started"
echo "Starting Silvana TEE Login Server"
/server
