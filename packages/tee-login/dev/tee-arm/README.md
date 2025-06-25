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
    "PCR0": "25877d6c4f524650d1c2ea65ca3dbba519c9c0123aebf1c4be5572f91ca9d3e641bd1125d30a35137d46fbd8377fbcb7",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "b4a931992afdfd637710119c8372561645f6c2c50712a0a7b112883458789faade6ba9fe736f46493f8e64c8c9cd61f5"
  }
}

Start building the Enclave Image...
Using the locally available Docker image...
Enclave Image successfully created.
{
  "Measurements": {
    "HashAlgorithm": "Sha384 { ... }",
    "PCR0": "f7c625578db6b8f0161a5e39900a98898f8ade65ae5670af293819bba2fa0cec50c5a97769ed61a83dbe3272b14a59c6",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "1e5d429ba31b0ac75156ccebd0a3a329ce429af8f3568b51d6f3618f11f2c144b72832a0173afd24c1eac84267e2b8d9"
  }
}


```
