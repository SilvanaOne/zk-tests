FROM amazonlinux:2023

# Install development tools, dependencies, and AWS CLI
RUN dnf update -y && \
    dnf groupinstall -y "Development Tools" && \
    dnf install -y git openssl-devel pkg-config protobuf-compiler protobuf-devel awscli

# Install Rust and Cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

# Set working directory
WORKDIR /app
# Copy Cargo.toml and Cargo.lock first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Copy proto files
COPY proto/ ./proto/

# Copy source code
COPY src/ ./src/

# Copy build.rs if it exists

COPY build.rs ./

# Build in release mode
RUN cargo build --release

# Create build directory and copy contents
RUN mkdir -p /app/rpc
COPY build/ /app/rpc/
RUN cp target/release/rpc /app/rpc/
RUN ls -lh /app/rpc/


# Create tar archive
RUN cd /app && tar -czf rpc.tar.gz -C /app rpc

# Upload to S3 

RUN --mount=type=secret,id=aws \
    export AWS_SHARED_CREDENTIALS_FILE=/run/secrets/aws && \
    echo $AWS_SHARED_CREDENTIALS_FILE && \
    aws s3 cp /app/rpc.tar.gz s3://silvana-tee-images/rpc.tar.gz