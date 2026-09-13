//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::{Podman, PodmanContainerState, PodmanError};

use {clap::Args, thiserror::Error};

/// Errors produced by the `enter` command.
#[derive(Debug, Error)]
pub enum EnterError {
    #[error(transparent)]
    Podman(#[from] PodmanError),

    #[error(
        "container '{name}' is in state '{state}', not 'running'; run `podcell start {name}` first"
    )]
    NotRunning {
        name: String,
        state: PodmanContainerState,
    },
}

/// Open an interactive shell in a running container.
#[derive(Args)]
pub struct Arguments {
    /// The name of the container to enter.
    #[arg()]
    name: String,
}

/// Handler for the "enter" command.
pub fn run(args: Arguments) -> Result<(), EnterError> {
    let podman = Podman::new();
    let container = podman.find_by_name(&args.name)?;

    if container.state != PodmanContainerState::Running {
        return Err(EnterError::NotRunning {
            name: args.name,
            state: container.state,
        });
    }

    podman.exec_interactive(&container.id, &["/usr/bin/podcell", "shell"])?;
    Ok(())
}
