guix pull
guix pack -f docker \
  --system=aarch64-linux \
  --target=aarch64-linux-musl \
  --entry-point=/bin/bash \
  -m manifest.scm \
  -S /bin/bash=bin/bash \
  -S /bin/sh=bin/bash \
  -S /bin/clang=bin/clang \
  -S /bin/cc=bin/clang      \
  -S /bin/lld=bin/lld       \
  -S /bin/gcc=bin/gcc       \
  -S /bin/ld=bin/ld         \
  -S /bin/rustc=bin/rustc   \
  -S /bin/git=bin/git       \
  -S /bin/pkg-config=bin/pkg-config \
  -S /bin/jq=bin/jq         \
  -S /bin/socat=bin/socat   \
  -S /bin/cpio=bin/cpio     \
  -S /bin/openssl=bin/openssl