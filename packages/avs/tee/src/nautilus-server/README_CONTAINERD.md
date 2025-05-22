# Containerd Implementation

This directory contains a Rust implementation for interacting with the [containerd](https://containerd.io/) container runtime.

## Prerequisites

Make sure you have containerd installed and running on your system:

```bash
# Check if containerd is installed
which containerd

# Check if containerd is running
systemctl status containerd
```

## Features

The implementation provides two main functions:

1. `load_container` - Loads container images from local tar files or remote registries
2. `run_container` - Creates, starts, and manages containers with timeout control

## Examples

You can find example code in the `examples` directory:

- `container_load.rs` - Demonstrates loading container images
- `container_run.rs` - Demonstrates running containers with timeout

## Running Examples

```bash
# Run the container loading example
cargo run --example container_load

# Run the container running example
cargo run --example container_run
```

## Implementation Details

- Uses the official containerd Rust client (`containerd-client` crate)
- Supports both local and remote image loading
- Automatically determines architecture for cross-platform compatibility
- Handles container lifecycle (create, start, wait, kill, delete)
- Implements timeouts for container execution

## Notes

- The containerd socket path is hardcoded to `/run/containerd/containerd.sock`
- The namespace is set to `default`
- For local image loading, a fully-featured implementation would require additional code to import tar files
