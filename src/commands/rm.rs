//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::{Podman, PodmanContainerState, PodmanError};

use {clap::Args, thiserror::Error};

/// Errors produced by the `rm` command.
#[derive(Debug, Error)]
pub enum RemoveError {
    #[error(transparent)]
    Podman(#[from] PodmanError),

    #[error("container '{name}' is in state '{state}'; run `podcell stop {name}` first")]
    Running {
        name: String,
        state: PodmanContainerState,
    },
}

/// Delete an existing container.
#[derive(Args)]
pub struct Arguments {
    /// The name of the container to delete.
    #[arg()]
    name: String,
}

/// Handler for the "rm" command.
pub fn run(args: Arguments) -> Result<(), RemoveError> {
    let podman = Podman::new();
    let container = podman.find_by_name(&args.name)?;

    if container.state == PodmanContainerState::Running {
        return Err(RemoveError::Running {
            name: args.name,
            state: container.state,
        });
    }

    podman.rm_by_id(&container.id)?;
    Ok(())
}
