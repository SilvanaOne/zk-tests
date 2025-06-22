# Silvana TEE login

This TEE configuration is based on Nautilus TEE configuration
https://github.com/MystenLabs/nautilus

## Create AWS keys

### AWS EC2

- AWS EC2 ED25519 KeyPair called TEE

### AWS KMS

AWS KMS KeyPair with the following configuration:

- Key Type: Symmetric
- Key Spec: SYMMETRIC_DEFAULT
- Key Usage: Encrypt and Decript
- Key material origin - KMS
- Name: TEEKMS
- User: silvana-tee-api-user TODO: refactor to create a user before KMS key

## Install AWS Stack with pulumi

```sh
brew install pulumi/tap/pulumi
pulumi version
pulumi login --local
cd pulumi
npm i
export AWS_ACCESS_KEY_ID=<YOUR_KEY_ID>
export AWS_SECRET_ACCESS_KEY=<YOUR_ACCESS_KEY>
export AWS_REGION=us-east-1
export PULUMI_CONFIG_PASSPHRASE=<CREATE_YOUR_PULUMI_PASSWORD>
pulumi up
pulumi stack output
```

Check deployment:

```sh
pulumi stack output --show-secrets
```

```
Current stack outputs (4):
    OUTPUT          VALUE
    apiAccessKeyId  ...
    apiSecretKey    ...
    bucketName      silvana-tee-login-b29fc12
    tableName       silvana-tee-login-7d0f6df
```

To start from scratch:

```sh
pulumi destroy
pulumi stack rm
```

## Connect with AWS instance

```sh
ssh -i "TEE.pem" ec2-user@ec2-23-21-249-129.compute-1.amazonaws.com
```

and in terminal run

```sh
git clone https://github.com/SilvanaOne/zk-tests
cd zk-tests/packages/tee-login/tee
make
```

on update

```sh
cd ../../.. && git pull origin main && cd packages/tee-login/tee && rm -rf out
make
```

after make

```sh
make run-debug
```

or, in production

```sh
make run
```

and in other terminal

```sh
sh expose_enclave.sh
```

To stop enclave:

```sh
sh reset_enclave.sh
```

copy image to s3 and back:

```sh
tar -czvf out.tar.gz out
aws s3 cp out.tar.gz s3://silvana-tee-images/tee.tar.gz
aws s3 cp s3://silvana-tee-images/tee.tar.gz tee.tar.gz
tar -xzvf out.tar.gz
```

curl -H 'Content-Type: application/json' -X GET http://23.21.249.129:3000/health_check

curl -H 'Content-Type: application/json' -X GET http://23.21.249.129:3000/stats

curl -H 'Content-Type: application/json' -X GET http://23.21.249.129:3000/get_attestation

curl -H 'Content-Type: application/json' -d '{"payload": { "memo": "agent"}}' -X POST http://54.242.34.226:3000/login

curl -H 'Content-Type: application/json' -d '{"payload": { "memo": "hi"}}' -X POST http://23.21.249.129:3000/ping

docker manifest inspect --verbose stagex/core-binutils:sx \
 | grep arm64 -B1 -A4

skopeo inspect --raw docker://stagex/core-binutils:sx \
 | jq -r '.manifests[] | select(.platform.architecture=="arm64") | .digest'

# multi-arch tag (both amd64 & arm64)

docker manifest inspect --verbose \
 ghcr.io/siderolabs/stagex/core-binutils:latest

core-ca-certificates

docker manifest inspect --verbose \
 ghcr.io/siderolabs/stagex/user-libseccomp:latest

# Docker Hub still lists them, even though the images are on GHCR

curl -s https://hub.docker.com/v2/repositories/stagex/core-binutils/tags/ \
 | jq '."results"[]|.name' | head

docker manifest inspect ghcr.io/siderolabs/stagex/user-libseccomp:latest \
 | jq -r '.manifests[]
| select(.platform.architecture=="arm64")
| .digest'

docker manifest inspect --verbose \
 ghcr.io/siderolabs/stagex/core-binutils:latest \
 | jq -r '.manifests[]
| select(.platform.architecture=="arm64")
| .digest'

TARGET="Containerfile"
SOURCE="https://codeberg.org/stagex/stagex/raw/branch/main/digests"
STAGES="core user bootstrap"

TMPFILE="$(mktemp)"

DIGESTS_TMP="$(mktemp)"
for stage in $STAGES; do
    curl -fsSL "$SOURCE/$stage.txt" | while read -r digest name; do
        echo "$name $digest" # >> "$DIGESTS_TMP"
done
done

curl -fsSL "https://codeberg.org/stagex/stagex/raw/branch/main/digests/core.txt"

# one-liner using skopeo; replace package name as needed

skopeo inspect --raw docker://ghcr.io/siderolabs/stagex/core-binutils:latest \
 | jq -r '.manifests[] | select(.platform.architecture=="arm64") | .digest'

skopeo inspect --raw docker://quay.io/stagex/core-libffi:latest \
 | jq -r '.manifests[] | select(.platform.architecture=="arm64") | .digest'

docker manifest inspect --verbose \
 quay.io/stagex/core-binutils:latest

docker manifest inspect --verbose \
 quay.io/stagex/core-libffi@sha256:9acd18e59ca11fa727670725e69a976d96f85a00704dea6ad07870bff2bd4e8b

core-libffi:sx2025.06.0
docker pull quay.io/stagex/core-libffi@sha256:9acd18e59ca11fa727670725e69a976d96f85a00704dea6ad07870bff2bd4e8b

ghcr.io/siderolabs/stagex
docker manifest inspect --verbose \
 ghcr.io/siderolabs/stagex/core-libffi

docker manifest inspect --verbose \
 quay.io/stagex/core-libffi

docker manifest inspect --verbose \
 stagex/core-clang

docker manifest inspect --verbose \
 ghcr.io/siderolabs/stagex/core-clang

skopeo inspect --raw docker://quay.io/stagex/core-clang:latest \
 | jq -r '.manifests[] | select(.platform.architecture=="arm64") | .digest'

skopeo inspect --raw docker://quay.io/stagex/core-clang:latest \
 | jq -r '.manifests[]
| [.platform.os,
.platform.architecture,
(.platform.variant // ""), # prints variant if present (e.g. v8)
.digest]
| @tsv'

skopeo inspect --raw docker://quay.io/stagex/core-clang@sha256:abf6d2868bc441b5910ef28f38123c6053391521948b33eaf68980fb8be7d105 \
 | jq -r '.manifests[]
| [.platform.os,
.platform.architecture,
(.platform.variant // ""), # prints variant if present (e.g. v8)
.digest]
| @tsv'

# arm64 child manifest (immutable)

FROM quay.io/stagex/core-libffi@sha256:9acd18e59ca11fa727670725e69a976d96f85a00704dea6ad07870bff2bd4e8b AS core-libffi

# one-off build on ANY host (x86, Arm, CI runner, etc.)

docker buildx build \
 --platform linux/arm64 \
 -t hello-libffi-arm64 \
 .

# run the tiny arm64 image (works on Apple Silicon or Graviton parent EC2)

docker run --rm hello-libffi-arm64

# → hello from libffi + clang on arm64!

# GUIX

https://guix.gnu.org/manual/en/html_node/Installation.html

```
;; manifest.scm  (pin whatever commit you want with `guix pull --commit=…`)
(specifications->manifest
 (list
   "clang-toolchain"   ; Clang + lld + compiler-rt
   "libffi"            ; you used it in the demo program
   "bash-minimal"
   "coreutils"))       ; convenient basics
```

guix pack -f docker \
 --system=aarch64-linux \
 --entry-point=/bin/bash \
 -m manifest.scm \
 -S /bin/bash=bin/bash \
 -S /bin/sh=bin/bash \
 -S /bin/clang=bin/clang

# (add more like -S /bin/lld=bin/lld if your build requires them)

docker load < /gnu/store/kp2zcf06d346irszwqsla02hnfwvaxnl-clang-toolchain-libffi-bash-minimal-docker-pack.tar.gz
docker tag $(docker images -q | head -n1) guix/clang:aarch64

Dockerfile:

```
FROM guix/clang:aarch64 AS builder
WORKDIR /src
COPY hello.c .
RUN ["/bin/clang", "-static", "-O2", "-s", "-o", "hello", "hello.c"]

FROM scratch
COPY --from=builder /src/hello /hello
ENTRYPOINT ["/hello"]
```

docker images guix/clang:aarch64

cat > hello.c <<'EOF'
#include <stdio.h>
int main(void) { puts("hello from libffi + clang on arm64!"); }
EOF

docker buildx build --platform=linux/arm64 -t hello-arm64 --load .
docker run --rm --platform=linux/arm64 hello-arm64
