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
    "PCR0": "1959d7f86d56868edb73f0b48c5142aed637e228aa9db037038c181963fc15557f0d280e752d27c62cefa80fd21c46af",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "591b44e0690f4519aa6a27e69dee6faa2f698898494f544b51aa0324454ac606c80eba7a29031a5195e689ad45d261fb"
  }
}
Start building the Enclave Image...
Using the locally available Docker image...
Enclave Image successfully created.
{
  "Measurements": {
    "HashAlgorithm": "Sha384 { ... }",
    "PCR0": "1959d7f86d56868edb73f0b48c5142aed637e228aa9db037038c181963fc15557f0d280e752d27c62cefa80fd21c46af",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "591b44e0690f4519aa6a27e69dee6faa2f698898494f544b51aa0324454ac606c80eba7a29031a5195e689ad45d261fb"
  }
}
```
