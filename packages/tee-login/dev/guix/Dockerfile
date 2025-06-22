# ------------------------------------------------------------------------
# tools + musl rootfs  (replaces every core-* and user-* you pulled)
FROM guix/base-tools:aarch64 AS guix-base
# ------------------------------------------------------------------------

FROM scratch as base
ENV TARGET=aarch64-unknown-linux-musl
ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV CARGOFLAGS="--locked --no-default-features --release --target ${TARGET}"
ENV OPENSSL_STATIC=true

COPY --from=guix-base / /