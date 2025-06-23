#!/bin/bash
# Update the instance and install Nitro Enclaves tools, Docker and other utilities
sudo yum update -y
sudo yum install -y aws-nitro-enclaves-cli-devel aws-nitro-enclaves-cli awscli docker nano socat git make

# Add the current user to the docker group (so you can run docker without sudo)
sudo usermod -aG docker ec2-user
sudo usermod -aG ne ec2-user

# Start and enable Nitro Enclaves allocator and Docker services
sudo systemctl start nitro-enclaves-allocator.service && sudo systemctl enable nitro-enclaves-allocator.service
sudo systemctl start docker && sudo systemctl enable docker
sudo systemctl enable nitro-enclaves-vsock-proxy.service
echo "- {address: dynamodb.us-east-1.amazonaws.com, port: 443}" | sudo tee -a /etc/nitro_enclaves/vsock-proxy.yaml
echo "- {address: kms.us-east-1.amazonaws.com, port: 443}" | sudo tee -a /etc/nitro_enclaves/vsock-proxy.yaml
echo "- {address: www.googleapis.com, port: 443}" | sudo tee -a /etc/nitro_enclaves/vsock-proxy.yaml
echo "- {address: api.github.com, port: 443}" | sudo tee -a /etc/nitro_enclaves/vsock-proxy.yaml

# Stop the allocator so we can modify its configuration
sudo systemctl stop nitro-enclaves-allocator.service

# Adjust the enclave allocator memory
ALLOCATOR_YAML=/etc/nitro_enclaves/allocator.yaml
MEM_KEY=memory_mib
DEFAULT_MEM=2096
CPU_KEY=cpu_count
DEFAULT_CPU=1
sudo sed -i "s/^${MEM_KEY}:.*/${MEM_KEY}: ${DEFAULT_MEM}/" "$ALLOCATOR_YAML"
sudo sed -i "s/^${CPU_KEY}:.*/${CPU_KEY}: ${DEFAULT_CPU}/" "$ALLOCATOR_YAML"

# Restart the allocator with the updated memory configuration
sudo systemctl start nitro-enclaves-allocator.service && sudo systemctl enable nitro-enclaves-allocator.service

# Restart vsock-proxy processes for various endpoints.
vsock-proxy 8101 dynamodb.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8102 kms.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8103 www.googleapis.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8104 api.github.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &



