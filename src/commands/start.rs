//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::{
    host::{current_user_uid, validate_host_path},
    podman::{Podman, PodmanContainerState},
};

use clap::Args;

/// Start an existing container without attaching to it.
#[derive(Args)]
pub struct Arguments {
    /// The name of the container to start.
    #[arg()]
    name: String,
}

/// Handler for the "start" command.
pub fn run(args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    let podman = Podman::new();
    let container = podman.find_by_name(&args.name)?;

    if container.state == PodmanContainerState::Running {
        println!("Container '{}' is already running.", args.name);
        return Ok(());
    }

    let user_uid = current_user_uid()?;
    let mount_sources = podman.list_user_bind_mount_sources(&container.id)?;
    for source in &mount_sources {
        validate_host_path(source, user_uid).map_err(|err| {
            format!(
                "Refusing to start '{}': mount source '{}' failed validation: {err}",
                args.name,
                source.display()
            )
        })?;
    }

    podman.start(&container.id).map_err(Into::into)
}
