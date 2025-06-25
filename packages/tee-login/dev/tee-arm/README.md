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
    "PCR0": "a4a57a53b277f40f374d37e9752c327f27a9a98e2b502737caf57666a3bf8f2700d2b677a8b179287e971900241a3e5f",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "f36b6a0c0d0a66d07887f61f8c0943c4901203e90768440bc9fa160396bfe555638411ffbf181f66c25265d769e6fdaa"
  }
}
Start building the Enclave Image...
Using the locally available Docker image...
Enclave Image successfully created.
{
  "Measurements": {
    "HashAlgorithm": "Sha384 { ... }",
    "PCR0": "fe5aba8921eed0c895c970b659abe6a98ecaec639067260dfee93f046ad526bc2af60786d2be0a6136aea29b2ef36b35",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "7705e534db96784892aff58a71575177e7556bac4378f6277d13e4cfb82032aa34e9819369143b83c8cace90192eedf2"
  }
}
```
