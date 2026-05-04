//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::{group::EtcGroup, package_manager::PackageManager, passwd::EtcPasswd};

use std::{
    fs, io,
    os::unix::{
        fs::{chown, MetadataExt},
        process::CommandExt,
    },
    path::Path,
    process::Command,
};

use clap::Args;

/// Path of the file used to remember the container initialization state.
const DEVSHELL_INIT_STATE_FILE_NAME: &str = "/.devshell";

/// Initialize the current environment.
#[derive(Args)]
pub struct Arguments {}

/// Prints a message in bold text.
fn print_bold(message: &str) {
    println!("\x1b[1m{message}\x1b[0m");
}

/// Container initialization procedure.
fn initialize() -> std::io::Result<()> {
    let username = std::env::var("USERNAME").map_err(|error| {
        io::Error::other(format!(
            "Failed to access the USERNAME environment variable: {error:?}"
        ))
    })?;

    let user_id = std::env::var("USER_ID").map_err(|error| {
        io::Error::other(format!(
            "Failed to access the USER_ID environment variable: {error:?}"
        ))
    })?;

    let group_id = std::env::var("GROUP_ID").map_err(|error| {
        io::Error::other(format!(
            "Failed to access the GROUP_ID environment variable: {error:?}"
        ))
    })?;

    let group_name = std::env::var("GROUP_NAME").map_err(|error| {
        io::Error::other(format!(
            "Failed to access the GROUP_NAME environment variable: {error:?}"
        ))
    })?;

    print_bold("Installing the required packages");
    let package_manager = PackageManager::new()?;
    package_manager.update()?;
    package_manager.install(["sudo", "bash"])?;

    let etc_passwd =
        EtcPasswd::new("/etc/passwd").map_err(|error| io::Error::other(format!("{error}")))?;

    if let Some(conflicting_user) = etc_passwd.iter().find_map(|user| {
        if format!("{}", user.id) == user_id {
            Some(user.name.clone())
        } else {
            None
        }
    }) {
        print_bold("Deleting conflicting user");

        // Don't pass --remove: with --userns=keep-id podman injects the host user into
        // /etc/passwd with HOME=/, and userdel --remove would then try to wipe `/`. We're
        // about to call useradd --create-home which builds a fresh /home/$USERNAME anyway,
        // so leaving any stray image-default home directory in place is fine.
        let status = Command::new("userdel")
            .args(["--force", &conflicting_user])
            .status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "Failed to delete conflicting user: {conflicting_user}",
            )));
        }
    }

    let etc_group =
        EtcGroup::new("/etc/group").map_err(|error| io::Error::other(format!("{error}")))?;

    if let Some(conflicting_group) = etc_group.iter().find_map(|group| {
        if format!("{}", group.id) == group_id {
            Some(group.name.clone())
        } else {
            None
        }
    }) {
        print_bold("Deleting conflicting group");

        let status = Command::new("groupdel")
            .args(["--force", &conflicting_group])
            .status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "Failed to delete conflicting group: {conflicting_group}",
            )));
        }
    }

    print_bold("Creating the primary group");

    let status = Command::new("groupadd")
        .args(["--gid", &group_id, &group_name])
        .status()?;

    if !status.success() {
        return Err(io::Error::other(format!(
            "Failed to create group: {group_name}",
        )));
    }

    print_bold("Creating the user");

    let sudo_group_name = etc_group
        .iter()
        .find_map(|group| {
            if group.name == "wheel" || group.name == "sudo" || group.name == "admin" {
                Some(group.name.clone())
            } else {
                None
            }
        })
        .ok_or(io::Error::other("Failed to locate the sudo/wheel group"))?;

    let status = Command::new("useradd")
        .arg("--create-home")
        .args(["--groups", &sudo_group_name])
        .args(["--uid", &user_id])
        .args(["--gid", &group_id])
        .args(["--shell", "/usr/bin/bash"])
        .arg(&username)
        .status()?;

    if !status.success() {
        return Err(io::Error::other(format!(
            "Failed to create user: {username}",
        )));
    }

    print_bold("Initializing the user password");

    let status = Command::new("bash")
        .args([
            "-c",
            &format!("set -ex ; set -o pipefail ; printf '{username}\\n{username}\\n' | passwd {username}"),
        ])
        .status()?;

    if !status.success() {
        return Err(io::Error::other(format!(
            "Failed to set the user password: {username}",
        )));
    }

    print_bold("Initializing the user folder");

    let home_path = format!("/home/{username}");
    let status = Command::new("cp")
        .args(["-r", "/etc/skel/.", &home_path])
        .status()?;

    if !status.success() {
        return Err(io::Error::other(format!(
            "Failed to copy /etc/skel to home directory: {home_path}",
        )));
    }

    // If a `--mount` destination lives under /home/$USERNAME, podman pre-creates the
    // parent directories as root before init runs. `useradd --create-home` then sees
    // /home/$USERNAME already exists and skips both the directory creation AND the
    // skel copy. The cp above also runs as root, so the skel-derived dotfiles end up
    // root-owned. Recursively chown the home tree to fix both at once, but stay on
    // the home dir's filesystem so we don't try to chown into bind mounts (where we'd
    // either get EPERM under the user namespace or, worse, mutate host file ownership).
    let user_id_n: u32 = user_id.parse().map_err(|err| {
        io::Error::other(format!("USER_ID is not a valid u32: '{user_id}': {err}"))
    })?;
    let group_id_n: u32 = group_id.parse().map_err(|err| {
        io::Error::other(format!("GROUP_ID is not a valid u32: '{group_id}': {err}"))
    })?;
    chown_tree_xdev(Path::new(&home_path), user_id_n, group_id_n)?;

    print_bold("The initialization has completed!");
    std::fs::File::create(DEVSHELL_INIT_STATE_FILE_NAME)?;

    Ok(())
}

/// Recursively chowns `root` and everything beneath it to `uid:gid`, but stops at
/// filesystem boundaries, in order to avoid changing mounted folders.
fn chown_tree_xdev(root: &Path, uid: u32, gid: u32) -> io::Result<()> {
    let root_dev = fs::symlink_metadata(root)?.dev();
    chown_walker(root, root_dev, uid, gid)
}

fn chown_walker(path: &Path, root_dev: u64, uid: u32, gid: u32) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.dev() != root_dev {
        // Different filesystem (bind mount, tmpfs, ...): skip entirely.
        return Ok(());
    }

    chown(path, Some(uid), Some(gid))?;

    if metadata.file_type().is_dir() {
        for entry in fs::read_dir(path)? {
            chown_walker(&entry?.path(), root_dev, uid, gid)?;
        }
    }

    Ok(())
}

/// Handler for the "init" command.
///
/// On first run, performs container initialization and exits cleanly so the user can
/// `devshell start` it later. On subsequent runs (state file present), execs `sleep infinity`
/// so the container has a long-lived PID 1 child that catatonit can SIGTERM cleanly when the
/// user runs `devshell stop`.
pub fn run(_args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(DEVSHELL_INIT_STATE_FILE_NAME).exists() {
        initialize().map_err(Into::into)
    } else {
        Err(Command::new("sleep").arg("infinity").exec().into())
    }
}
