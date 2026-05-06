//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use std::{io, os::unix::process::CommandExt, process::Command};

use clap::Args;

/// Open an interactive login shell inside the container.
///
/// Hidden subcommand: invoked by `podcell enter` via `podman exec`. Reads `USERNAME` from
/// the container's environment (set at create time) and execs `sudo -H -i -u $USERNAME`.
#[derive(Args)]
pub struct Arguments {}

/// Handler for the "shell" command.
pub fn run(_args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    let username = std::env::var("USERNAME").map_err(|err| {
        io::Error::other(format!(
            "Failed to access the USERNAME environment variable: {err}"
        ))
    })?;

    Err(Command::new("sudo")
        .arg("-H")
        .arg("-i")
        .arg("-u")
        .arg(&username)
        .exec()
        .into())
}
