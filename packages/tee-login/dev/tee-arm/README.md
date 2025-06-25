# Example of reproducable build for TEE

docker pull --platform linux/arm64 rust:1.87-alpine  
docker inspect --format='{{ index .RepoDigests 0 }}' rust:1.87-alpine
docker pull --platform linux/arm64 alpine:3.22
docker inspect --format='{{ index .RepoDigests 0 }}' alpine:3.22
docker pull --platform linux/arm64 public.ecr.aws/amazonlinux/amazonlinux:2023
docker buildx imagetools inspect \
 public.ecr.aws/amazonlinux/amazonlinux:2023 \
 --format '{{json .Manifest}}' | \
 jq '.manifests[] | select(.platform.architecture=="arm64") | .digest'

```
Start building the Enclave Image...
Using the locally available Docker image...
Enclave Image successfully created.
{
  "Measurements": {
    "HashAlgorithm": "Sha384 { ... }",
    "PCR0": "814526ee9cc1a4a681ba50cf32276781aa73aa2281bba7fb392d4ead7e19acdd1d0c52a0c26210ef45bfed59dd444b57",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "d94b9a034b87a42a09f60033cfe147a84532f5368a2c688c6e58701a625518d9b523fdc05c182575f0718fe45e0c1ce4"
  }
}

```
