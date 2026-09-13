# podcell

A simple Podman-based development environment manager.

**podcell** is a personal utility I've primarily written for myself to quickly create, manage, and enter isolated development containers using Podman. It aims to provide a minimal and straightforward command-line interface for managing development environments.

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
podcell create fedora:42 mybox \
    --mount .:/mnt/work:rw \
    --mount /tmp/cache:/mnt/cache:ro

# Start it.
podcell start mybox

# Open a shell. Run from multiple terminals concurrently.
podcell enter mybox

# Send a file or directory into the container (must be running).
podcell send mybox /path/to/file

# Stop it (terminates all open shells).
podcell stop mybox

# Remove it (must be stopped first).
podcell rm mybox
```

The `--mount` flag takes `HOST:CONTAINER[:MODE]` where `MODE` is `ro` or `rw` (default `ro`).
The `CONTAINER` path must be absolute.

The `HOST` path may be relative: it is resolved against the current directory of the
`podcell create` invocation and recorded in the container configuration as an absolute path, so
`podcell start` works from any directory.

`podcell send` copies a file or directory into `/inbox` inside the container. The folder is created
on first use with mode `0777` and its contents belong to the container's primary user, so items can
be read, edited, moved and deleted from inside.

### Mount path requirements

- **The host path must be owned by the user running `podcell`.** Mounts owned by root or
  another user are rejected at create and start time. This is required for the rootless
  userns mapping to work correctly and to keep SELinux relabel side effects confined to
  user-owned data.
- **Avoid mounting paths whose SELinux labels matter to the host.** Bind mounts are passed
  to podman with the `:z` flag, which recursively relabels the host path tree to a shared
  container label. This is fine for project directories, scratch space, etc.; it will
  break consumers of paths with load-bearing labels such as `~/.ssh`, `~/.config/dconf`,
  `~/.local/share/keyrings`, and similar. Don't pass those as `--mount`.
- **Relative host paths are frozen at create time.** A relative `HOST` is canonicalized during
  `podcell create`, so moving the container does not retarget the mount: if you want a different
  source path, remove and recreate the container.

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
ldd -d target/x86_64-unknown-linux-musl/release/podcell
```

The output should show "not a dynamic executable" or similar, confirming static linking.

### Why Static Linking?

The `podcell` binary must be statically linked because it executes itself inside containers where dynamic dependencies may not be available. Static linking ensures the binary runs in any Linux environment without external dependencies.
