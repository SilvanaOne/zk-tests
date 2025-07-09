# Dockerfile

# Stage 1: Build the Rust binary for aarch64-musl
FROM rust:1.88 AS builder

# Add the musl target
RUN rustup target add aarch64-unknown-linux-musl

WORKDIR /app
# Copy only the manifest to leverage layer caching
COPY Cargo.toml Cargo.lock ./
# Copy source code
COPY src/ src/

# Build in release mode for the musl target
RUN cargo build --release --target aarch64-unknown-linux-musl

# Stage 2: Create minimal image with the static binary
FROM scratch

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/aarch64-unknown-linux-musl/release/musl /usr/local/bin/app

# Define entrypoint
ENTRYPOINT ["/usr/local/bin/app"]

