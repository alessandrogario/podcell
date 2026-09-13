//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::{Podman, PodmanContainerState, PodmanError};

use clap::Args;

/// Stop a running container.
#[derive(Args)]
pub struct Arguments {
    /// The name of the container to stop.
    #[arg()]
    name: String,
}

/// Handler for the "stop" command.
pub fn run(args: Arguments) -> Result<(), PodmanError> {
    let podman = Podman::new();
    let container = podman.find_by_name(&args.name)?;

    if container.state != PodmanContainerState::Running {
        println!(
            "Container '{}' is in state '{}', nothing to stop.",
            args.name, container.state
        );
        return Ok(());
    }

    podman.stop(&container.id)
}
