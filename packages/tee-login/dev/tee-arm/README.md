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
    "PCR0": "0391507214bce7b2e4c017a3fc3e1b1b9df0de7de0dce6c38a87db0c0286c06fd6261dac0702032cfb97a583afee224c",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "fd9747e6d5011a0112f22bd59c23cee72e9094bdd6cb7e45891a361bf04918ed0a2617432c23f9da308d80c33d3e8df2"
  }
}
Start building the Enclave Image...
Using the locally available Docker image...
Enclave Image successfully created.
{
  "Measurements": {
    "HashAlgorithm": "Sha384 { ... }",
    "PCR0": "4dafa37307d292e73db1590dbcd26da12e6f438c6f7ffca1f4d12d61063f0f460eba207e266d85e912e22d93be3d78bb",
    "PCR1": "3b4a7e1b5f13c5a1000b3ed32ef8995ee13e9876329f9bc72650b918329ef9cf4e2e4d1e1e37375dab0ba56ba0974d03",
    "PCR2": "ca3da70894517bc9021aa979a628f6f9b0b095b6e43782e8d2480d6ed1f200767505f88127ce08f1f6538132b8322ddb"
  }
}
```
