//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::{
    host::{current_user_uid, validate_host_path},
    mount::Mount,
    podman::Podman,
};

use std::path::{Path, PathBuf};

use clap::Args;

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

    /// Bind mount in the form HOST:CONTAINER[:MODE]. MODE is `ro` or `rw` (default `ro`).
    /// Pass --mount multiple times to add multiple mounts.
    #[arg(
        long = "mount",
        value_name = "HOST:CONTAINER[:MODE]",
        help = "Bind mount a host path into the container (repeatable)"
    )]
    mounts: Vec<Mount>,
}

/// Handler for the "create" command.
pub fn run(args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    let user_uid = current_user_uid()?;

    let mut prepared_mounts: Vec<Mount> = Vec::with_capacity(args.mounts.len());

    for mount in args.mounts {
        let host = prepare_host_path(&mount.host, user_uid)?;
        prepared_mounts.push(Mount {
            host,
            container: mount.container,
            mode: mount.mode,
        });
    }

    Podman::new()
        .create(
            args.host_network,
            &prepared_mounts,
            &args.distribution,
            &args.name,
        )
        .map_err(Into::into)
}

/// Validate and canonicalize a user-supplied host path.
///
/// Strict semantics: the path must already exist (we do not auto-create), it must be
/// owned by the current user, and after canonicalization it must not contain any `:`
/// characters (which would be misparsed by podman's `--volume HOST:CONTAINER:MODE`
/// argument
fn prepare_host_path(host: &Path, user_uid: u32) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !host.exists() {
        return Err(format!(
            "Mount host path '{}' does not exist. Create it before running `podcell create`.",
            host.display()
        )
        .into());
    }

    validate_host_path(host, user_uid)?;

    let canonical = host.canonicalize().map_err(|e| {
        format!(
            "Failed to canonicalize mount host path '{}': {e}",
            host.display()
        )
    })?;

    if let Some(s) = canonical.to_str() {
        if s.contains(':') {
            return Err(format!(
                "Mount host path '{}' canonicalizes to '{s}', which contains ':' \
                 and would be misparsed by podman's --volume argument.",
                host.display()
            )
            .into());
        }
    } else {
        return Err(format!(
            "Mount host path '{}' canonicalizes to a non-UTF-8 path",
            host.display()
        )
        .into());
    }

    // Re-validate ownership on the canonical path: a symlink might have pointed at a
    // path with different ownership than the link itself.
    validate_host_path(&canonical, user_uid)?;

    Ok(canonical)
}
