#!/bin/bash

# AWS EC2 User Data Script for Silvana RPC Server
# This script performs initial system setup and then calls start.sh from the git repository
# Designed to stay under AWS 16KB user-data limit

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

echo "Installing Rust and Cargo..."
sudo -u ec2-user -i bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
sudo -u ec2-user -i bash -c 'source ~/.cargo/env && rustc --version && cargo --version'

echo "Cloning zk-tests repository as ec2-user into /home/ec2-user..."
if command -v git >/dev/null 2>&1; then
    sudo -u ec2-user -i bash -c 'cd /home/ec2-user && git clone --quiet --no-progress --depth 1 https://github.com/SilvanaOne/zk-tests'
else
    echo "ERROR: Git is still not available after installation attempts"
    exit 1
fi

# Run the main setup script from the cloned repository
echo "Running Silvana RPC setup script..."
if [ -f "/home/ec2-user/zk-tests/packages/avs/rpc/pulumi/start.sh" ]; then
    sudo -u ec2-user bash /home/ec2-user/zk-tests/packages/avs/rpc/pulumi/start.sh
    setup_exit_code=$?
    if [ $setup_exit_code -eq 0 ]; then
        echo "✅ Silvana RPC setup completed successfully"
    else
        echo "❌ Silvana RPC setup failed with exit code: $setup_exit_code"
        echo "Check /var/log/start-script.log for detailed error information"
        exit 1
    fi
else
    echo "ERROR: start.sh script not found in cloned repository"
    echo "Expected location: /home/ec2-user/zk-tests/packages/avs/rpc/pulumi/start.sh"
    ls -la /home/ec2-user/zk-tests/packages/avs/rpc/pulumi/ || echo "Directory listing failed"
    exit 1
fi

echo "User-data script completed successfully at $(date)"

