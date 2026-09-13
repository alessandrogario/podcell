//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::{Podman, PodmanContainerState, PodmanError};

use std::path::PathBuf;

use clap::Args;

/// Path, inside the container, of the folder where sent items are placed.
const INBOX_PATH: &str = "/inbox";

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

/// Creates the inbox folder when it is missing and sets its mode to `0777`.
fn create_inbox(podman: &Podman, container_id: &str) -> Result<(), PodmanError> {
    podman.exec(
        container_id,
        &[
            "sh",
            "-c",
            &format!("mkdir -p {INBOX_PATH} && chmod 0777 {INBOX_PATH}"),
        ],
    )
}

/// Gives the inbox and its contents to the container's primary user.
///
/// `podman cp` preserves the numeric owner of the source, and `--userns=keep-id` maps the host
/// uid to uid 0 inside the container: copied items arrive owned by root.
fn chown_inbox(podman: &Podman, container_id: &str) -> Result<(), PodmanError> {
    podman.exec(
        container_id,
        &[
            "sh",
            "-c",
            &format!("chown -R \"$USER_ID:$GROUP_ID\" {INBOX_PATH}"),
        ],
    )
}

/// Handler for the "send" command.
pub fn run(args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    if !args.source.exists() {
        return Err(format!("Source path '{}' does not exist.", args.source.display()).into());
    }

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

    println!("Sending '{}' to /inbox...", args.source.display());
    create_inbox(&podman, &container.id)?;

    podman.cp(&container.id, &args.source, &format!("{INBOX_PATH}/"))?;
    chown_inbox(&podman, &container.id)?;

    Ok(())
}
