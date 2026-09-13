//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use std::{io, os::unix::process::CommandExt, process::Command};

use {clap::Args, thiserror::Error};

/// Errors produced by the hidden `shell` command.
#[derive(Debug, Error)]
pub enum ShellError {
    #[error("failed to access the USERNAME environment variable: {0}")]
    Username(#[source] std::env::VarError),

    #[error("failed to execute the login shell: {0}")]
    Execute(#[source] io::Error),
}

/// Open an interactive login shell inside the container.
///
/// Hidden subcommand: invoked by `podcell enter` via `podman exec`. Reads `USERNAME` from
/// the container's environment (set at create time) and execs `sudo -H -i -u $USERNAME`.
#[derive(Args)]
pub struct Arguments {}

/// Handler for the "shell" command.
pub fn run(_args: Arguments) -> Result<(), ShellError> {
    let username = std::env::var("USERNAME").map_err(ShellError::Username)?;

    Err(ShellError::Execute(
        Command::new("sudo")
            .arg("-H")
            .arg("-i")
            .arg("-u")
            .arg(&username)
            .exec(),
    ))
}
