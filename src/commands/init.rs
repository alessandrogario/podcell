//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::{group::EtcGroup, package_manager::PackageManager, passwd::EtcPasswd};

use std::{io, os::unix::process::CommandExt, path::Path, process::Command};

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
            "Failed to access the USER_ID environment variable: {error:?}"
        ))
    })?;

    let group_name = std::env::var("GROUP_NAME").map_err(|error| {
        io::Error::other(format!(
            "Failed to access the USER_ID environment variable: {error:?}"
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

        let status = Command::new("userdel")
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .args(["--force", "--remove", &conflicting_user])
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
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
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
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
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
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
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
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
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
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .args(["-r", "/etc/skel/.", &home_path])
        .status()?;

    if !status.success() {
        return Err(io::Error::other(format!(
            "Failed to copy /etc/skel to home directory: {home_path}",
        )));
    }

    print_bold("The initialization has completed!");
    std::fs::File::create(DEVSHELL_INIT_STATE_FILE_NAME)?;

    Ok(())
}

/// Starts a new login session within the container.
fn start_session() -> std::io::Result<()> {
    let mut cmd = Command::new("sudo");
    cmd.stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .arg("-H")
        .arg("-i")
        .arg("-u")
        .arg(&std::env::var("USERNAME").map_err(|err| {
            io::Error::other(format!(
                "Failed to access the USERNAME environment variable: {err}"
            ))
        })?);

    Err(cmd.exec())
}

/// Handler for the "init" command.
pub fn run(_args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(DEVSHELL_INIT_STATE_FILE_NAME).exists() {
        initialize().map_err(Into::into)
    } else {
        start_session().map_err(Into::into)
    }
}
