# devshell

A simple Podman-based development environment manager.

**devshell** is a personal utility I've primarily written for myself to quickly create, manage, and enter isolated development containers using Podman. It aims to provide a minimal and straightforward command-line interface for managing development environments.

**This tool is intentionally simple and designed for personal workflows on the atomic Linux distribution I'm using.**

## Features

- Create new containers for development, with arbitrary repeatable bind mounts
- Start and stop containers as a discrete lifecycle step
- Enter a running container from multiple terminals at once
- Send files and directories into a running container via `/inbox`
- List all available containers
- Remove containers when no longer needed

## Usage

```sh
# Create the container (runs initialization, then exits to state Exited).
devshell create fedora:42 mybox \
    --mount $PWD:/mnt/work:rw \
    --mount /tmp/cache:/mnt/cache:ro

# Start it.
devshell start mybox

# Open a shell. Run from multiple terminals concurrently.
devshell enter mybox

# Send a file or directory into the container (must be running).
devshell send mybox /path/to/file

# Stop it (terminates all open shells).
devshell stop mybox

# Remove it (must be stopped first).
devshell rm mybox
```

The `--mount` flag takes `HOST:CONTAINER[:MODE]` where `MODE` is `ro` or `rw` (default `ro`).
Both paths must be absolute.

`devshell send` copies a file or directory into `/inbox` inside the container. The `/inbox`
directory is created automatically on first use with sticky world-writable permissions (`1777`),
and sent items are made world-readable and writable after copying.

### Mount path requirements

- **The host path must be owned by the user running `devshell`.** Mounts owned by root or
  another user are rejected at create and start time. This is required for the rootless
  userns mapping to work correctly and to keep SELinux relabel side effects confined to
  user-owned data.
- **Avoid mounting paths whose SELinux labels matter to the host.** Bind mounts are passed
  to podman with the `:z` flag, which recursively relabels the host path tree to a shared
  container label. This is fine for project directories, scratch space, etc.; it will
  break consumers of paths with load-bearing labels such as `~/.ssh`, `~/.config/dconf`,
  `~/.local/share/keyrings`, and similar. Don't pass those as `--mount`.

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
