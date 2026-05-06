//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::{Podman, PodmanContainerState};

use clap::Args;

/// Open an interactive shell in a running container.
#[derive(Args)]
pub struct Arguments {
    /// The name of the container to enter.
    #[arg()]
    name: String,
}

/// Handler for the "enter" command.
pub fn run(args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    let podman = Podman::new();
    let container = podman.find_by_name(&args.name)?;

    if container.state != PodmanContainerState::Running {
        return Err(format!(
            "Container '{name}' is in state '{state}', not 'running'. \
             Run `podcell start {name}` first.",
            name = args.name,
            state = container.state,
        )
        .into());
    }

    podman
        .exec_interactive(&container.id, &["/usr/bin/podcell", "shell"])
        .map_err(Into::into)
}
