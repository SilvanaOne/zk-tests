cd ../../../.. && git pull origin main && cd packages/tee-login/tee/arm && rm -rf out && make

curl -H 'Content-Type: application/json' -X GET http://35.175.45.79:3000/health_check

curl -H 'Content-Type: application/json' -X GET http://54.82.214.64:3000/stats
curl -H 'Content-Type: application/json' -X GET https://tee2.silvana.dev/stats

curl -H 'Content-Type: application/json' -X GET http://35.175.45.79:3000/get_attestation

run:
ssh -i "TEE.pem" ec2-user@35.175.45.79
dev:
ssh -i "TEE.pem" ec2-user@54.82.214.64
sudo less /var/log/cloud-init-output.log

```sh
git clone https://github.com/SilvanaOne/zk-tests
cd zk-tests/packages/tee-login/tee/arm
make
```

copy image to s3 and back:

```sh
tar -czvf tee-arm-v6.tar.gz out
aws s3 cp tee-arm-v6.tar.gz s3://silvana-tee-images/tee-arm-v6.tar.gz
aws s3 cp time-server-v1.tar.gz s3://silvana-tee-images/time-server-v1.tar.gz
aws s3 cp s3://silvana-tee-images/tee-arm-v6.tar.gz tee-arm-v6.tar.gz
tar -xzvf tee-arm-v6.tar.gz
aws s3 cp s3://silvana-tee-images/time-server-v1.tar.gz time-server-v1.tar.gz
tar -xzvf time-server-v1.tar.gz
```

Job for nitro-enclaves-allocator.service failed because the control process exited with error code.
See "systemctl status nitro-enclaves-allocator.service" and "journalctl -xeu nitro-enclaves-allocator.service" for details.

"Token has expired: claim.iat:1750893282 claim.exp:1750896882 now:1750917839"

# Requires Docker ≥ 23 with BuildKit / buildx

docker buildx imagetools inspect --raw docker.io/library/rust:1.87-alpine \
 | jq -r '.manifests[]
| select(.platform.os=="linux" and .platform.architecture=="arm64")
| .digest'

docker buildx imagetools inspect --raw docker.io/library/alpine:3.22 \
 | jq -r '.manifests[]
| select(.platform.os=="linux" and .platform.architecture=="arm64")
| .digest'

docker buildx imagetools inspect --raw docker.io/library/amazonlinux:2023 \
 | jq -r '.manifests[]
| select(.platform.os=="linux" and .platform.architecture=="arm64")
| .digest'
sha256:2ae982a3cc43011aaf80f42b086451c0c562a319b2e020e089f35338dfda1360

cargo generate-lockfile
mkdir -p vendor
cargo vendor vendor/ --versioned-dirs

```
Start building the Enclave Image...
Using the locally available Docker image...
{
  "Measurements": {
    "HashAlgorithm": "Sha384 { ... }",
    "PCR0": "a7123f036c74fa7d92c105180e1b42608a921e57a219bae4065b6df55a1a5342693077ae9744ee93622cd0833769bb9e",
    "PCR1": "745004eab9a0fb4a67973b261c6e7fa5418dc870292927591574385649338e54686cdeb659f3c6c2e72ba11aba2158a8",
    "PCR2": "56031752985921fa72af0c56fe8685aeea9d2bb5823788625c4eaba2d8919680a5950d9281446a146dc6459df24fdf2f"
  }
}
```
