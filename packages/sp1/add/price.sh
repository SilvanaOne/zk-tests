#!/bin/bash
# SP1 Price Prover - Standalone Runner
# This script runs the SP1 price prover Docker container without needing the full repo

set -e

# Configuration
DOCKER_IMAGE="${DOCKER_IMAGE:-docker.io/dfstio/price-devnet-test:latest}"
PROOFS_DIR="${PROOFS_DIR:-./proofs}"

# Default parameters
TOKEN="${TOKEN:-BTC}"
COUNT="${COUNT:-1}"
INTERVAL="${INTERVAL:-5}"
SYSTEM="${SYSTEM:-groth16}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Function to show usage
usage() {
    cat << EOF
SP1 Price Prover - Docker Runner

USAGE:
    ./price.sh [OPTIONS]

OPTIONS:
    --token TOKEN          Token symbol (default: BTC)
    --count COUNT          Number of price points to fetch (default: 1)
    --interval SECONDS     Interval between fetches in seconds (default: 5)
    --system SYSTEM        Proof system: groth16, plonk, or compressed (default: groth16)
    --image IMAGE          Docker image to use (default: docker.io/dfstio/price-devnet-test:latest)
    --proofs-dir DIR       Directory to store proofs (default: ./proofs)
    --pull                 Force pull latest image before running
    --execute              Run in execute mode (no proof generation)
    --help                 Show this help message

SECURITY NOTE:
    Groth16 and PLONK proofs require Docker-in-Docker functionality.
    This script mounts /var/run/docker.sock into the container, which gives
    the container access to your Docker daemon. Only use with trusted images.

    For compressed proofs (no Docker socket access), use: --system compressed

EXAMPLES:
    # Generate single BTC Groth16 proof (default)
    ./price.sh --token BTC

    # Generate 5 ETH price proofs with 3-second intervals
    ./price.sh --token ETH --count 5 --interval 3

    # Generate PLONK proof instead of Groth16
    ./price.sh --token BTC --system plonk

    # Generate compressed core proof (for aggregation)
    ./price.sh --token ETH --system compressed

    # Execute without generating proof (faster, for testing)
    ./price.sh --token SOL --execute

    # Pull latest image and generate proof
    ./price.sh --pull --token BTC --count 3

ENVIRONMENT VARIABLES:
    DOCKER_IMAGE           Docker image to use
    TOKEN                  Token symbol
    COUNT                  Number of price points
    INTERVAL               Interval in seconds
    SYSTEM                 Proof system (groth16, plonk, compressed)
    PROOFS_DIR             Directory for proof output

OUTPUT:
    Proofs are saved to: ${PROOFS_DIR}/
    - price-core-proof-<timestamp>.json (Groth16/PLONK/Compressed)

EOF
}

# Parse command line arguments
MODE="prove"
PULL=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --token)
            TOKEN="$2"
            shift 2
            ;;
        --count)
            COUNT="$2"
            shift 2
            ;;
        --interval)
            INTERVAL="$2"
            shift 2
            ;;
        --system)
            SYSTEM="$2"
            shift 2
            ;;
        --image)
            DOCKER_IMAGE="$2"
            shift 2
            ;;
        --proofs-dir)
            PROOFS_DIR="$2"
            shift 2
            ;;
        --pull)
            PULL=true
            shift
            ;;
        --execute)
            MODE="execute"
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Header
echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║           SP1 Price Prover - Docker Runner                   ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed!"
    echo ""
    echo "Install Docker:"
    echo "  Ubuntu/Debian: curl -fsSL https://get.docker.com | sh"
    echo "  Or visit: https://docs.docker.com/engine/install/"
    exit 1
fi

print_success "Docker is installed"

# Create proofs directory
mkdir -p "$PROOFS_DIR"
print_success "Proofs directory ready: $PROOFS_DIR"

# Pull image if requested
if [ "$PULL" = true ]; then
    print_info "Pulling Docker image: $DOCKER_IMAGE"
    if docker pull "$DOCKER_IMAGE"; then
        print_success "Image pulled successfully"
    else
        print_error "Failed to pull image"
        exit 1
    fi
fi

# Check if image exists locally
if ! docker image inspect "$DOCKER_IMAGE" &> /dev/null; then
    print_warning "Image not found locally, pulling from registry..."
    if docker pull "$DOCKER_IMAGE"; then
        print_success "Image pulled successfully"
    else
        print_error "Failed to pull image: $DOCKER_IMAGE"
        echo ""
        echo "Make sure the image is available or use --image to specify a different one"
        exit 1
    fi
fi

# Display configuration
echo ""
print_info "Configuration:"
echo "  Image:     $DOCKER_IMAGE"
echo "  Token:     $TOKEN"
echo "  Count:     $COUNT"
echo "  Interval:  ${INTERVAL}s"
echo "  System:    $SYSTEM"
echo "  Mode:      $MODE"
echo "  Output:    $PROOFS_DIR/"
echo ""

# Prepare Docker command
DOCKER_CMD="docker run --rm"

# Set Rust logging level
DOCKER_CMD="$DOCKER_CMD -e RUST_LOG=info"

# Mount Docker socket for Groth16/PLONK proofs (Docker-in-Docker)
# Skip for compressed proofs or execute mode
if [ "$MODE" = "prove" ] && [ "$SYSTEM" != "compressed" ]; then
    DOCKER_CMD="$DOCKER_CMD -v /var/run/docker.sock:/var/run/docker.sock"

    # Mount SP1 circuits directory for sharing with nested gnark container
    # This allows DinD to access circuit files downloaded by the parent container
    mkdir -p "$HOME/.sp1/circuits"
    mkdir -p "$HOME/.sp1/tmp"
    DOCKER_CMD="$DOCKER_CMD -v $HOME/.sp1:/root/.sp1"

    # Set TMPDIR to shared path for Docker-in-Docker temp file access
    # Temp files (witness, output) must be accessible to both containers and host
    DOCKER_CMD="$DOCKER_CMD -e TMPDIR=/root/.sp1/tmp"

    # Set SP1 circuit paths to use host paths for Docker-in-Docker
    # This ensures the gnark container can find circuits on the host filesystem
    DOCKER_CMD="$DOCKER_CMD -e SP1_GROTH16_CIRCUIT_PATH=$HOME/.sp1/circuits/groth16"
    DOCKER_CMD="$DOCKER_CMD -e SP1_PLONK_CIRCUIT_PATH=$HOME/.sp1/circuits/plonk"

    print_warning "Mounting Docker socket for ${SYSTEM} proof generation"
    print_info "Caching SP1 circuits in: $HOME/.sp1/circuits"
    print_info "Using shared temp directory: $HOME/.sp1/tmp"
fi

DOCKER_CMD="$DOCKER_CMD -v $(pwd)/$PROOFS_DIR:/app/proofs"
DOCKER_CMD="$DOCKER_CMD $DOCKER_IMAGE"

if [ "$MODE" = "execute" ]; then
    DOCKER_CMD="$DOCKER_CMD --execute"
else
    DOCKER_CMD="$DOCKER_CMD --prove --system $SYSTEM"
fi

DOCKER_CMD="$DOCKER_CMD --token $TOKEN --count $COUNT --interval $INTERVAL"

# Run the container
print_info "Starting price prover container..."
echo ""
echo "⏱️  Start time: $(date '+%Y-%m-%d %H:%M:%S')"
START_TIME=$(date +%s)

if eval $DOCKER_CMD; then
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))

    echo ""
    echo "⏱️  End time: $(date '+%Y-%m-%d %H:%M:%S')"
    echo "⏱️  Duration: ${DURATION}s"
    echo ""
    print_success "Price prover completed successfully!"

    # Show generated files
    if [ "$MODE" = "prove" ]; then
        echo ""
        print_info "Generated files:"
        ls -lh "$PROOFS_DIR"/*.json 2>/dev/null | tail -5 || echo "  No proof files found"
    fi

    exit 0
else
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))

    echo ""
    echo "⏱️  End time: $(date '+%Y-%m-%d %H:%M:%S')"
    echo "⏱️  Duration: ${DURATION}s"
    echo ""
    print_error "Price prover failed!"
    exit 1
fi
