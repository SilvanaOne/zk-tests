docker pull --platform linux/arm64 rust:1.87-alpine  
docker inspect --format='{{ index .RepoDigests 0 }}' rust:1.87-alpine
docker pull --platform linux/arm64 alpine:3.22
docker inspect --format='{{ index .RepoDigests 0 }}' alpine:3.22
docker pull --platform linux/arm64 public.ecr.aws/amazonlinux/amazonlinux:2023
docker buildx imagetools inspect \
 public.ecr.aws/amazonlinux/amazonlinux:2023 \
 --format '{{json .Manifest}}' | \
 jq '.manifests[] | select(.platform.architecture=="arm64") | .digest'
