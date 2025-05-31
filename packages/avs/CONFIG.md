export ENCLAVE_PACKAGE_ID=0x27f81d3f6e6f39035f2e72641b9163e19a96561b158c888ba4217e3d06fa5f1f
export ENCLAVE_CAP_OBJECT_ID=0xbf188afd24301eac1940e31fec756e157cf60d3a5b93ae4396fd1c3d75534a13
export CAP_OBJECT_ID=0xcea47534f03b6170a8722e61a435abd1b3b274aa6a379522a7777423795accd7
export ENCLAVE_CONFIG_OBJECT_ID=0x8c60c4f2b947a7c31968a176944c0a4d7d38de57db6677de7f344ffd818ba060
export AGENT_PACKAGE_ID=0xd3713d13ac58a7fac28593ad4cde3ec5fede563b82688fb4b0b16898320817ab
export MODULE_NAME=agent
export OTW_NAME=AGENT
export ENCLAVE_URL=http://54.242.34.226:3000

sui client call --function update_name --module enclave --package $ENCLAVE_PACKAGE_ID --type-args "$AGENT_PACKAGE_ID::$MODULE_NAME::$OTW_NAME" --args $ENCLAVE_CONFIG_OBJECT_ID $CAP_OBJECT_ID "agent enclave, updated 2025-05-21"

curl -H 'Content-Type: application/json' -X GET http://54.242.34.226:3000/health_check

curl -H 'Content-Type: application/json' -X GET http://54.242.34.226:3000/get_attestation

curl -H 'Content-Type: application/json' -d '{"payload": { "memo": "agent"}}' -X POST http://54.242.34.226:3000/process_data

# make sure you have jq installed (`brew install jq`)

docker manifest inspect stagex/user-docker:latest \
 | jq -r '.manifests[]
| select(.platform.architecture=="amd64")
| .digest'

sudo systemctl status nitro-enclaves-vsock-proxy.service
sudo systemctl status nitro-enclaves-allocator.service

# Restart vsock-proxy processes for various endpoints.

vsock-proxy 8101 fullnode.devnet.sui.io 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8102 fullnode.testnet.sui.io 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8103 dex.silvana.dev 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8104 hub.docker.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8105 registry-1.docker.io 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8106 auth.docker.io 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
vsock-proxy 8107 docker.io 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &

rm -rf out && cd ../../.. && git pull origin main && cd packages/avs/tee
