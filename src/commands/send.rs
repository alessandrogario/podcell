//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::{Podman, PodmanContainerState};

use std::path::PathBuf;

use clap::Args;

/// Send a file or directory into a running container's /inbox.
#[derive(Args)]
pub struct Arguments {
    /// The name of the container to send to.
    #[arg()]
    name: String,

    /// Path to the file or directory to send.
    #[arg()]
    source: PathBuf,
}

/// Handler for the "send" command.
pub fn run(args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    if !args.source.exists() {
        return Err(format!("Source path '{}' does not exist.", args.source.display()).into());
    }

    let item_name = args
        .source
        .file_name()
        .ok_or("source path has no file name")?
        .to_str()
        .ok_or("source file name is not valid UTF-8")?;

    let podman = Podman::new();
    let container = podman.find_by_name(&args.name)?;

    if container.state != PodmanContainerState::Running {
        return Err(format!(
            "Container '{name}' is in state '{state}', not 'running'. \
             Run `devshell start {name}` first.",
            name = args.name,
            state = container.state,
        )
        .into());
    }

    podman.exec(
        &container.id,
        &["sh", "-c", "mkdir -p /inbox && chmod 1777 /inbox"],
    )?;

    println!("Sending '{}' to /inbox...", args.source.display());
    podman.cp(&container.id, &args.source, "/inbox/")?;

    podman.exec(
        &container.id,
        &["chmod", "-R", "a+rwX", &format!("/inbox/{item_name}")],
    )?;

    Ok(())
}
