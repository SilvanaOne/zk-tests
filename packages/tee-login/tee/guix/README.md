sh build.sh
docker load < gnu-pack.tar.gz
docker tag $(docker images -q | head -n1) guix/tee:aarch64

docker buildx build \
 --platform=linux/arm64 \
 -t tee:aarch64 \
 --load .
