#!/bin/sh
echo "Starting containerd"
mkdir -p /run/containerd
/usr/local/bin/containerd --config /etc/containerd/config.toml &