//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::{
    group::{EtcGroup, EtcGroupError},
    package_manager::{PackageManager, PackageManagerError},
    passwd::{EtcPasswd, EtcPasswdError},
    podman::{Config, Podman, PodmanError},
};

use {clap::Args, thiserror::Error};

use std::{
    fs, io,
    num::ParseIntError,
    os::unix::{
        fs::{MetadataExt, chown},
        process::CommandExt,
    },
    path::Path,
    process::{Command, ExitStatus},
};

/// Path of the file used to remember the container initialization state.
const PODCELL_INIT_STATE_FILE_NAME: &str = "/.podcell";

/// Errors produced while initializing a container.
#[derive(Debug, Error)]
pub enum InitError {
    #[error("failed to access the {name} environment variable: {source}")]
    EnvironmentVariable {
        name: &'static str,
        #[source]
        source: std::env::VarError,
    },

    #[error(transparent)]
    PackageManager(#[from] PackageManagerError),

    #[error(transparent)]
    Passwd(#[from] EtcPasswdError),

    #[error(transparent)]
    Group(#[from] EtcGroupError),

    #[error("failed to execute {command}: {source}")]
    ExecuteCommand {
        command: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("{command} failed for '{target}' (exit_status: {exit_status:?})")]
    CommandFailed {
        command: &'static str,
        target: String,
        exit_status: ExitStatus,
    },

    #[error("failed to locate the sudo/wheel group")]
    SudoGroupNotFound,

    #[error("{name} is not a valid u32: '{value}': {source}")]
    InvalidId {
        name: &'static str,
        value: String,
        #[source]
        source: ParseIntError,
    },

    #[error(transparent)]
    ChownTree(#[from] ChownTreeError),

    #[error("failed to create initialization state file '{path}': {source}")]
    CreateStateFile {
        path: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("failed to execute the container keepalive process: {0}")]
    ExecuteKeepalive(#[source] io::Error),
}

/// Errors produced while creating the temporary base image used by `edit`.
#[derive(Debug, Error)]
pub enum GenerateContainerBaseImageError {
    #[error("failed to commit container before editing: {0}")]
    Commit(#[source] PodmanError),

    #[error("failed to create temporary image build directory '{path}': {source}")]
    CreateBuildDirectory {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to write temporary Dockerfile '{path}': {source}")]
    WriteDockerfile {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to build edited container image: {0}")]
    Build(#[source] PodmanError),

    #[error("failed to remove temporary image build directory '{path}': {source}")]
    RemoveBuildDirectory {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to remove temporary container image: {0}")]
    RemoveImage(#[source] PodmanError),
}

/// Errors produced while recursively changing ownership of the initialized home directory.
#[derive(Debug, Error)]
pub enum ChownTreeError {
    #[error("failed to read metadata for '{path}': {source}")]
    Metadata {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to change ownership of '{path}': {source}")]
    Chown {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to read directory '{path}': {source}")]
    ReadDirectory {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to read an entry in directory '{path}': {source}")]
    ReadDirectoryEntry {
        path: String,
        #[source]
        source: io::Error,
    },
}

/// Initialize the current environment.
#[derive(Args)]
pub struct Arguments {}

/// Prints a message in bold text.
fn print_bold(message: &str) {
    println!("\x1b[1m{message}\x1b[0m");
}

/// Container initialization procedure.
fn initialize() -> Result<(), InitError> {
    let read_environment_variable = |name| {
        std::env::var(name).map_err(|source| InitError::EnvironmentVariable { name, source })
    };

    let username = read_environment_variable("USERNAME")?;
    let user_id = read_environment_variable("USER_ID")?;
    let group_id = read_environment_variable("GROUP_ID")?;
    let group_name = read_environment_variable("GROUP_NAME")?;

    print_bold("Installing the required packages");
    let package_manager = PackageManager::new()?;
    package_manager.update()?;
    package_manager.install(["sudo", "bash"])?;

    let etc_passwd = EtcPasswd::new("/etc/passwd")?;

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
            .status()
            .map_err(|source| InitError::ExecuteCommand {
                command: "userdel",
                source,
            })?;

        if !status.success() {
            return Err(InitError::CommandFailed {
                command: "userdel",
                target: conflicting_user,
                exit_status: status,
            });
        }
    }

    let etc_group = EtcGroup::new("/etc/group")?;

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
            .status()
            .map_err(|source| InitError::ExecuteCommand {
                command: "groupdel",
                source,
            })?;

        if !status.success() {
            return Err(InitError::CommandFailed {
                command: "groupdel",
                target: conflicting_group,
                exit_status: status,
            });
        }
    }

    print_bold("Creating the primary group");

    let status = Command::new("groupadd")
        .args(["--gid", &group_id, &group_name])
        .status()
        .map_err(|source| InitError::ExecuteCommand {
            command: "groupadd",
            source,
        })?;

    if !status.success() {
        return Err(InitError::CommandFailed {
            command: "groupadd",
            target: group_name.clone(),
            exit_status: status,
        });
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
        .ok_or(InitError::SudoGroupNotFound)?;

    let status = Command::new("useradd")
        .arg("--create-home")
        .args(["--groups", &sudo_group_name])
        .args(["--uid", &user_id])
        .args(["--gid", &group_id])
        .args(["--shell", "/usr/bin/bash"])
        .arg(&username)
        .status()
        .map_err(|source| InitError::ExecuteCommand {
            command: "useradd",
            source,
        })?;

    if !status.success() {
        return Err(InitError::CommandFailed {
            command: "useradd",
            target: username.clone(),
            exit_status: status,
        });
    }

    print_bold("Initializing the user password");

    let status = Command::new("bash")
        .args([
            "-c",
            &format!("set -ex ; set -o pipefail ; printf '{username}\\n{username}\\n' | passwd {username}"),
        ])
        .status()
        .map_err(|source| InitError::ExecuteCommand {
            command: "passwd",
            source,
        })?;

    if !status.success() {
        return Err(InitError::CommandFailed {
            command: "passwd",
            target: username.clone(),
            exit_status: status,
        });
    }

    print_bold("Initializing the user folder");

    let home_path = format!("/home/{username}");
    let status = Command::new("cp")
        .args(["-r", "/etc/skel/.", &home_path])
        .status()
        .map_err(|source| InitError::ExecuteCommand {
            command: "cp",
            source,
        })?;

    if !status.success() {
        return Err(InitError::CommandFailed {
            command: "cp",
            target: home_path.clone(),
            exit_status: status,
        });
    }

    // If a `--mount` destination lives under /home/$USERNAME, podman pre-creates the
    // parent directories as root before init runs. `useradd --create-home` then sees
    // /home/$USERNAME already exists and skips both the directory creation AND the
    // skel copy. The cp above also runs as root, so the skel-derived dotfiles end up
    // root-owned. Recursively chown the home tree to fix both at once, but stay on
    // the home dir's filesystem so we don't try to chown into bind mounts (where we'd
    // either get EPERM under the user namespace or, worse, mutate host file ownership).
    let user_id_n: u32 = user_id.parse().map_err(|source| InitError::InvalidId {
        name: "USER_ID",
        value: user_id,
        source,
    })?;

    let group_id_n: u32 = group_id.parse().map_err(|source| InitError::InvalidId {
        name: "GROUP_ID",
        value: group_id,
        source,
    })?;

    chown_tree_xdev(Path::new(&home_path), user_id_n, group_id_n)?;

    print_bold("The initialization has completed!");
    std::fs::File::create(PODCELL_INIT_STATE_FILE_NAME).map_err(|source| {
        InitError::CreateStateFile {
            path: PODCELL_INIT_STATE_FILE_NAME,
            source,
        }
    })?;

    Ok(())
}

/// Builds an image without `/.podcell` so the recreated container initializes and exits.
pub fn generate_container_base_image(
    podman: &Podman,
    config: &Config,
) -> Result<(), GenerateContainerBaseImageError> {
    let temp_image_ref = format!("localhost/podcell:{}-edit", config.name);
    podman
        .commit(&config.name, &temp_image_ref)
        .map_err(GenerateContainerBaseImageError::Commit)?;

    let temp_dir = std::env::temp_dir().join(format!("podcell-edit-{}", config.name));
    let dockerfile = format!(
        "FROM {temp_image_ref}\nRUN echo 'Deinitializing the container...' && rm -f {PODCELL_INIT_STATE_FILE_NAME}\n"
    );

    std::fs::create_dir_all(&temp_dir).map_err(|source| {
        GenerateContainerBaseImageError::CreateBuildDirectory {
            path: temp_dir.display().to_string(),
            source,
        }
    })?;
    let dockerfile_path = temp_dir.join("Dockerfile");
    std::fs::write(&dockerfile_path, dockerfile).map_err(|source| {
        GenerateContainerBaseImageError::WriteDockerfile {
            path: dockerfile_path.display().to_string(),
            source,
        }
    })?;

    podman
        .build(&temp_dir, &config.image_ref)
        .map_err(GenerateContainerBaseImageError::Build)?;
    std::fs::remove_dir_all(&temp_dir).map_err(|source| {
        GenerateContainerBaseImageError::RemoveBuildDirectory {
            path: temp_dir.display().to_string(),
            source,
        }
    })?;

    podman
        .rmi(&temp_image_ref)
        .map_err(GenerateContainerBaseImageError::RemoveImage)?;
    Ok(())
}

/// Recursively chowns `root` and everything beneath it to `uid:gid`, but stops at
/// filesystem boundaries, in order to avoid changing mounted folders.
fn chown_tree_xdev(root: &Path, uid: u32, gid: u32) -> Result<(), ChownTreeError> {
    let root_dev = fs::symlink_metadata(root)
        .map_err(|source| ChownTreeError::Metadata {
            path: root.display().to_string(),
            source,
        })?
        .dev();
    chown_walker(root, root_dev, uid, gid)
}

fn chown_walker(path: &Path, root_dev: u64, uid: u32, gid: u32) -> Result<(), ChownTreeError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ChownTreeError::Metadata {
        path: path.display().to_string(),
        source,
    })?;
    if metadata.dev() != root_dev {
        // Different filesystem (bind mount, tmpfs, ...): skip entirely.
        return Ok(());
    }

    chown(path, Some(uid), Some(gid)).map_err(|source| ChownTreeError::Chown {
        path: path.display().to_string(),
        source,
    })?;

    if metadata.file_type().is_dir() {
        let entries = fs::read_dir(path).map_err(|source| ChownTreeError::ReadDirectory {
            path: path.display().to_string(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| ChownTreeError::ReadDirectoryEntry {
                path: path.display().to_string(),
                source,
            })?;
            chown_walker(&entry.path(), root_dev, uid, gid)?;
        }
    }

    Ok(())
}

/// Handler for the "init" command.
///
/// On first run, performs container initialization and exits cleanly so the user can
/// `podcell start` it later. On subsequent runs (state file present), execs `sleep infinity`
/// so the container has a long-lived PID 1 child that catatonit can SIGTERM cleanly when the
/// user runs `podcell stop`.
pub fn run(_args: Arguments) -> Result<(), InitError> {
    if !Path::new(PODCELL_INIT_STATE_FILE_NAME).exists() {
        initialize()
    } else {
        Err(InitError::ExecuteKeepalive(
            Command::new("sleep").arg("infinity").exec(),
        ))
    }
}
