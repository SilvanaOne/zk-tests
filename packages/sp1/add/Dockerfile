# syntax=docker/dockerfile:1

###############################
# Stage 1: Builder
# Uses pre-built SP1 base image with toolchain already installed
###############################
FROM docker.io/dfstio/sp1-builder:latest AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock rust-toolchain ./

# Copy all crate sources
COPY lib ./lib/
COPY programs ./programs/
COPY script ./script/
COPY witness ./witness/
COPY sui ./sui/

# Build only the price binary in release mode
RUN cd script && cargo build --release --bin price

###############################
# Stage 2: Runtime
###############################
FROM debian:bookworm-slim

# Install runtime dependencies + Docker CLI for Groth16 proof generation
# Docker CLI is needed because SP1's Groth16 prover runs gnark circuits in Docker containers
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    docker.io \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/price /usr/local/bin/price

# Copy pre-built ELF programs (needed by SP1 SDK at runtime)
COPY --from=builder /app/target/elf-compilation /app/elf

# Set environment for ELF location
ENV SP1_ELF_DIR=/app/elf/riscv32im-succinct-zkvm-elf/release

# Create proofs output directory
RUN mkdir -p /app/proofs

# Set working directory for output
WORKDIR /app

# Default entrypoint
ENTRYPOINT ["price"]

# Default shows help
CMD ["--help"]
