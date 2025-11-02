# syntax=docker/dockerfile:1

###############################
# SP1 Builder Base Image
# This image contains the SP1 toolchain and is intended to be reused
# across multiple SP1 projects to avoid reinstalling the toolchain
###############################

FROM rust:1.88-bookworm

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install SP1 toolchain with non-interactive mode
# This installs:
# - SP1 CLI tools in ~/.sp1/bin/
# - succinct Rust toolchain via rustup
# - cargo-prove for zkVM compilation
ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    RUSTUP_INIT_SKIP_PATH_CHECK=yes

RUN set -x && \
    echo "=== Step 1: Installing sp1up CLI ===" && \
    curl -L https://sp1.succinct.xyz | bash && \
    echo "=== Step 2: Verifying sp1up installation ===" && \
    ls -la /root/.sp1/bin/ && \
    file /root/.sp1/bin/sp1up && \
    echo "=== Step 3: Running sp1up to install SP1 toolchain (10 min timeout) ===" && \
    timeout 600 /root/.sp1/bin/sp1up 2>&1 | tee /tmp/sp1up.log; \
    EXIT_CODE=$?; \
    echo "sp1up exit code: $EXIT_CODE"; \
    if [ $EXIT_CODE -eq 124 ]; then \
        echo "=== sp1up TIMED OUT after 10 minutes ===" && \
        cat /tmp/sp1up.log && \
        ps aux && \
        exit 1; \
    elif [ $EXIT_CODE -ne 0 ]; then \
        echo "=== sp1up FAILED with exit code $EXIT_CODE ===" && \
        cat /tmp/sp1up.log && \
        exit 1; \
    fi && \
    echo "=== Step 4: Verifying cargo-prove installation ===" && \
    /root/.sp1/bin/cargo-prove prove --version

# Add SP1 to PATH for all future commands
ENV PATH="/root/.sp1/bin:${PATH}"

# Install stable toolchain with required components for building the host script
# The workspace rust-toolchain file requires stable with llvm-tools and rustc-dev
RUN echo "=== Installing stable toolchain with components ===" && \
    rustup toolchain install stable && \
    rustup component add llvm-tools rustc-dev --toolchain stable && \
    rustup default stable && \
    echo "=== Toolchains available ===" && \
    rustup toolchain list

# Set working directory for derived images
WORKDIR /app

# Display installed versions for verification
RUN echo "=== SP1 Builder Image ===" && \
    rustc --version && \
    cargo --version && \
    cargo-prove prove --version && \
    echo "========================"
