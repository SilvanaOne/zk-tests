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

docker manifest inspect stagex/user-libseccomp:latest \
 | jq -r '.manifests[]
| select(.platform.architecture=="amd64")
| .digest'

docker manifest inspect stagex/user-docker:latest \
 | jq -r '.manifests[]
| select(.platform.architecture=="amd64")
| .digest'

docker manifest inspect stagex/user-fuse-overlayfs:latest \
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
vsock-proxy 8108 127.0.0.1 5000 --config /etc/nitro_enclaves/vsock-proxy.yaml &

docker run -d --restart=always --name mirror \
 -p 5000:5000 \
 -e REGISTRY_PROXY_REMOTEURL=https://registry-1.docker.io \
 registry:2

rm -rf out && cd ../../.. && git pull origin main && cd packages/avs/tee

# /etc/nitro_enclaves/vsock-proxy.yaml on the **host**

docker pull --platform=linux/amd64 dfstio/testagent2:latest
id=$(docker create --platform=linux/amd64 dfstio/testagent2:latest)
docker export "$id" | gzip > testagent2-rootfs.tar.gz # rootfs = one layer
docker rm "$id"

docker pull --platform=linux/arm64 dfstio/testagent2:latest
id=$(docker create --platform=linux/arm64 dfstio/testagent2:latest)
docker export "$id" | gzip > out/testagent2.tar.gz
docker rm "$id"

docker pull --platform=linux/amd64 dfstio/testagent2:latest
docker save --platform=linux/amd64 dfstio/testagent2:latest | gzip > testagent2-image.tar.gz

docker pull --platform=linux/arm64 dfstio/testagent2:latest
docker save --platform=linux/arm64 dfstio/testagent2:latest | gzip > testagent2.tar.gz

docker import \
 --change 'CMD ["npm","run","start"]' \
 --change 'WORKDIR /app' \
 testagent2.tar.gz dfstio/testagent2:flat
docker login
docker push dfstio/testagent2:flat

docker pull --platform=linux/amd64 dfstio/testagent4:latest
id=$(docker create --platform=linux/amd64 dfstio/testagent4:latest)
docker export "$id" | gzip > out/testagent4-amd64.tar.gz
docker rm "$id"
docker import \
 --change 'CMD ["npm","run","start"]' \
 --change 'WORKDIR /app' \
 out/testagent4-amd64.tar.gz dfstio/testagent4:flat-amd64
docker login
docker push dfstio/testagent4:flat-amd64

65.109.109.52:8301
docker run \
 --rm \
 --name mina-snark-worker \
 --network host \
 -e RAYON_NUM_THREADS=4 \
 gcr.io/o1labs-192920/mina-daemon:3.1.2-alpha1-e8b0893-focal-devnet \
 internal snark-worker \
 --daemon-address 65.109.109.52:8301 \
 --proof-level full \
 --shutdown-on-disconnect true
