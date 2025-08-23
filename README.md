# devshell

A simple Podman-based development environment manager.

**devshell** is a personal utility I've primarily written for myself to quickly create, manage, and enter isolated development containers using Podman. It aims to provide a minimal and straightforward command-line interface for managing development environments.

**This tool is intentionally simple and designed for personal workflows on the atomic Linux distribution I'm using.**

## Features

- Create new containers for development
- Enter existing containers with ease
- List all available containers
- Remove containers when no longer needed

## Build Instructions

### Prerequisites

Install the musl C library development tools:
```sh
# Ubuntu/Debian
sudo apt-get install musl-tools

# Fedora
sudo dnf install musl-devel
```

### Building

1. Add the musl target to your Rust toolchain:
   ```sh
   rustup target add x86_64-unknown-linux-musl
   ```

2. Build the project with static linking:
   ```sh
   cargo build --release --target x86_64-unknown-linux-musl
   ```

### Verification

Verify the binary is statically linked:
```sh
ldd -d target/x86_64-unknown-linux-musl/release/devshell
```

The output should show "not a dynamic executable" or similar, confirming static linking.

### Why Static Linking?

The `devshell` binary must be statically linked because it executes itself inside containers where dynamic dependencies may not be available. Static linking ensures the binary runs in any Linux environment without external dependencies.
