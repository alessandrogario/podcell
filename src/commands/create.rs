//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::Podman;

use std::{os::unix::fs::PermissionsExt, path::PathBuf};

use clap::{arg, Args};

/// Create a new container.
#[derive(Args)]
pub struct Arguments {
    /// Distribution and version, in the following format: distro:version.
    #[arg()]
    distribution: String,

    /// Container name.
    #[arg()]
    name: String,

    /// If enabled, the container is created with --network=host.
    #[arg(long, default_value_t = false, help = "Use the host network namespace")]
    host_network: bool,

    /// Optional path to a shared folder to mount into the container.
    /// The folder will be mounted at /mnt/shared inside the container.
    #[arg(
        long,
        value_name = "PATH",
        help = "Path to a shared folder to mount into the container at /mnt/shared"
    )]
    shared_folder: Option<PathBuf>,
}

/// Handler for the "create" command.
pub fn run(args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(shared_folder) = &args.shared_folder {
        std::fs::create_dir_all(shared_folder)?;

        println!("\x1b[1mConfiguring shared folder permissions\x1b[0m");
        std::fs::set_permissions(shared_folder, std::fs::Permissions::from_mode(0o777))?;

        let user = std::env::var("USER")?;
        let status = std::process::Command::new("setfacl")
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .arg("-m")
            .arg(format!("u:{user}:rwx"))
            .arg(shared_folder)
            .status()?;

        if !status.success() {
            return Err(format!("Failed to set ACL on {shared_folder:?}").into());
        }
    }

    Podman::new()
        .create(
            args.host_network,
            args.shared_folder,
            &args.distribution,
            &args.name,
        )
        .map_err(Into::into)
}
