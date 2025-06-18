# Traffic forwarder

/etc/hosts in enclave:
127.0.0.64 dynamodb.us-east-1.amazonaws.com

/etc/nitro_enclaves/vsock-proxy.yaml in parent instance:

- {address: dynamodb.us-east-1.amazonaws.com, port: 443}

run inside enclave
/forwarder 127.0.0.64 443 3 8101 &

run in parent instance
sudo systemctl start nitro-enclaves-vsock-proxy.service
sudo systemctl enable nitro-enclaves-vsock-proxy.service
vsock-proxy 8101 dynamodb.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
