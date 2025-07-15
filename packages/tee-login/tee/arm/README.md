cd ../../../.. && git pull origin main && cd packages/tee-login/tee/arm && rm -rf out && make

curl -H 'Content-Type: application/json' -X GET http://35.175.45.79:3000/health_check

curl -H 'Content-Type: application/json' -X GET http://54.82.214.64:3000/stats
curl -H 'Content-Type: application/json' -X GET http://54.82.214.64:3000/get_attestation
curl -H 'Content-Type: application/json' -X GET http://54.82.214.64:3000/health_check
curl -H 'Content-Type: application/json' -X GET https://tee2.silvana.dev/stats

curl -H 'Content-Type: application/json' -X GET http://35.175.45.79:3000/get_attestation

curl -H 'Content-Type: application/json' -X GET https://tee2.silvana.dev/get_attestation
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
Output file: /workspace/out/tee-arm.eif
#43 0.910 Output file: /workspace/tee-arm.eif
#43 1.028 {
#43 1.028   "HashAlgorithm": "Sha384 { ... }",
#43 1.028   "PCR0": "7342cf561ca9f27b205a84eddab7fd0ba05ddaf5a7e6495561f2902483b710409f044190dd64ad5efe489a9b1eb21ec4",
#43 1.028   "PCR1": "7342cf561ca9f27b205a84eddab7fd0ba05ddaf5a7e6495561f2902483b710409f044190dd64ad5efe489a9b1eb21ec4",
#43 1.028   "PCR2": "21b9efbc184807662e966d34f390821309eeac6802309798826296bf3e8bec7c10edb30948c90ba67310f7b964fc500a"
#43 1.028 }
```
