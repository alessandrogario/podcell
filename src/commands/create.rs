//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::{
    mount::{Mount, parse_validated_user_mount},
    podman::{Config, Podman, PodmanError},
    port::Port,
};

use clap::Args;

/// Create a new container.
#[derive(Args)]
pub struct Arguments {
    /// Source image reference: a distro image in the `distro:version` format (e.g. `fedora:42`),
    /// or a committed/tagged container image.
    #[arg()]
    image_ref: String,

    /// Container name.
    #[arg()]
    name: String,

    /// Bind mount in the form HOST:CONTAINER[:MODE]. MODE is `ro` or `rw` (default `ro`).
    /// HOST may be relative, and is resolved against the current directory; CONTAINER must
    /// be absolute. The host path must already exist. Pass --mount multiple times to add
    /// multiple mounts.
    #[arg(
        long = "mount",
        value_name = "HOST:CONTAINER[:MODE]",
        help = "Bind mount a host path into the container (repeatable, relative paths allowed)",
        value_parser = parse_validated_user_mount
    )]
    mounts: Vec<Mount>,

    /// Publish a host port to the container in the form HOST:CONTAINER[/PROTOCOL].
    /// PROTOCOL is `tcp` or `udp` and defaults to `tcp`. Pass --ports multiple times to
    /// publish multiple ports.
    #[arg(
        long = "ports",
        value_name = "HOST:CONTAINER/PROTOCOL",
        help = "Publish a host port to the container (repeatable, defaults to TCP)"
    )]
    ports: Vec<Port>,
}

/// Creates a container from the supplied configuration.
pub fn run(args: Arguments) -> Result<(), PodmanError> {
    let config = Config {
        image_ref: args.image_ref,
        name: args.name,
        mounts: args.mounts,
        ports: args.ports,
    };

    Podman::new().create(&config)
}
