//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::{
    group::{EtcGroup, EtcGroupError},
    passwd::{EtcPasswd, EtcPasswdError},
};

use std::{
    env, io,
    os::unix::process::CommandExt,
    path::PathBuf,
    process::{Command, ExitStatus},
    string::FromUtf8Error,
};

use thiserror::Error;

/// The name of the Podman executable.
const PODMAN_EXECUTABLE_NAME: &str = "podman";

/// The path within the container where our own binary is mounted.
const DEVSHELL_MOUNT_PATH: &str = "/usr/bin/devshell";

/// Path to the /etc/passwd file.
const ETC_PASSWD_PATH: &str = "/etc/passwd";

/// Path to the /etc/group file.
const ETC_GROUP_PATH: &str = "/etc/group";

#[derive(Error, Debug)]
pub enum PodmanError {
    #[error("failed to execute podman")]
    IOError(#[from] io::Error),

    #[error("podman has returned invalid UTF-8 output")]
    InvalidOutputEncoding(#[from] FromUtf8Error),

    #[error("podman has returned invalid JSON output")]
    InvalidJSONOutput(#[from] serde_json::Error),

    #[error("the JSON returned by podman is missing a required field: {0}")]
    MissingJSONKey(String),

    #[error("podman exited with failure (key: {key_name:?}, expected_type: {expected_type:?})")]
    InvalidJSONKeyType {
        key_name: String,
        expected_type: String,
    },

    #[error("the following container is either missing or is not managed by devshell: {0}")]
    NotFound(String),

    #[error("podman exited with failure (exit_status: {0:?})")]
    CommandError(ExitStatus),

    #[error("group file error")]
    EtcGroupError(#[from] EtcGroupError),

    #[error("passwd file error")]
    EtcPasswdError(#[from] EtcPasswdError),
}

/// These states correspond to the lifecycle phases of a container as reported by Podman.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PodmanContainerState {
    /// The container has been created but not started.
    Created,

    /// The container is currently running.
    Running,

    /// The container is paused.
    Paused,

    /// The container has exited.
    Exited,

    /// The container is stopped.
    Stopped,

    /// The container is in the process of stopping.
    Stopping,

    /// The container is restarting.
    Restarting,

    /// The container is dead.
    Dead,
}

impl std::str::FromStr for PodmanContainerState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "created" => Ok(PodmanContainerState::Created),
            "running" => Ok(PodmanContainerState::Running),
            "paused" => Ok(PodmanContainerState::Paused),
            "exited" => Ok(PodmanContainerState::Exited),
            "stopped" => Ok(PodmanContainerState::Stopped),
            "stopping" => Ok(PodmanContainerState::Stopping),
            "restarting" => Ok(PodmanContainerState::Restarting),
            "dead" => Ok(PodmanContainerState::Dead),
            _ => Err(()),
        }
    }
}

/// Represents a container.
pub struct PodmanContainer {
    /// The container ID.
    pub id: String,

    /// The container image.
    pub image: String,

    /// The container image ID.
    pub image_id: String,

    /// The container names.
    pub name_list: Vec<String>,

    /// The container state.
    pub state: PodmanContainerState,
}

/// Represents an interface to the Podman command-line tool.
#[derive(Default)]
pub struct Podman;

impl Podman {
    /// Creates a new `Interface` instance.
    pub fn new() -> Self {
        Self {}
    }

    /// Returns a list of all containers managed by us.
    pub fn list(&self) -> Result<Vec<PodmanContainer>, PodmanError> {
        let podman_output = Command::new(PODMAN_EXECUTABLE_NAME)
            .args(["ps", "--all", "--format=json"])
            .stderr(std::process::Stdio::inherit())
            .output()?;

        if !podman_output.status.success() {
            return Err(PodmanError::CommandError(podman_output.status));
        }

        let json_output: serde_json::Value =
            serde_json::from_str(&String::from_utf8(podman_output.stdout)?)?;

        let mut container_list = Vec::new();

        for json_object in json_output
            .as_array()
            .ok_or(io::Error::other("The JSON output is not an array."))?
        {
            let label_map = Self::get_json_object_string_map(json_object, "Labels")?;
            if !label_map
                .iter()
                .any(|(key, value)| key == "manager" && value == "devshell")
            {
                continue;
            }

            let id = Self::get_json_object_string(json_object, "Id")?;
            let image = Self::get_json_object_string(json_object, "Image")?;
            let image_id = Self::get_json_object_string(json_object, "ImageID")?;
            let name_list = Self::get_json_object_string_list(json_object, "Names")?;
            let string_state = Self::get_json_object_string(json_object, "State")?;

            container_list.push(PodmanContainer {
                id,
                image,
                image_id,
                name_list,
                state: string_state
                    .parse()
                    .map_err(|_| io::Error::other("Invalid container state"))?,
            });
        }

        Ok(container_list)
    }

    /// Creates a new container.
    pub fn create(
        &self,
        use_host_network: bool,
        shared_folder: Option<PathBuf>,
        distribution: &str,
        name: &str,
    ) -> Result<(), PodmanError> {
        let mut cmd = Command::new("podman");

        cmd.stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .arg("run")
            .arg("--init")
            .arg("--tty")
            .arg("--interactive")
            .args(["--label", "manager=devshell"])
            .args(["--hosts-file", "image"])
            .arg(format!("--add-host={name}:127.0.0.1"))
            .arg(format!("--add-host={name}:::1"));

        if use_host_network {
            cmd.arg("--network=host");
        } else {
            cmd.arg("--network=pasta");
        }

        cmd.args(["--name", name])
            .args(["--hostname", name])
            .args(["--security-opt", "mask=/proc/acpi,/proc/kcore,/proc/keys"])
            .arg("--cap-drop=SYS_ADMIN,NET_ADMIN,SYS_MODULE,SYS_RAWIO,SYS_BOOT,MAC_ADMIN,MAC_OVERRIDE,SYSLOG,SYS_PTRACE,SYS_TIME")
            .arg("--cap-add=DAC_OVERRIDE,DAC_READ_SEARCH");

        let self_path = env::current_exe()?;
        let self_path_str = self_path.to_str().ok_or_else(|| {
            io::Error::other(format!(
                "Failed to convert the binary path '{}' to string",
                self_path.display()
            ))
        })?;

        cmd.args([
            "--volume",
            &format!("{self_path_str}:{DEVSHELL_MOUNT_PATH}:ro,z"),
        ])
        .args(["--entrypoint", DEVSHELL_MOUNT_PATH]);

        if let Some(shared_folder) = shared_folder {
            let shared_folder = shared_folder.canonicalize().map_err(|e| {
                io::Error::other(format!(
                    "Failed to canonicalize the shared folder path '{}': {e}",
                    shared_folder.display()
                ))
            })?;

            let shared_folder_str = shared_folder.to_str().ok_or_else(|| {
                io::Error::other(format!(
                    "Failed to convert the shared folder path '{}' to string",
                    shared_folder.display()
                ))
            })?;

            cmd.args(["--volume", &format!("{shared_folder_str}:/mnt/shared:rw")]);
        }

        match (
            std::path::Path::new("/sys/module/apparmor/parameters/enabled").exists(),
            std::path::Path::new("/sys/fs/selinux/enforce").exists(),
        ) {
            (true, false) => {
                cmd.args(["--security-opt", "apparmor=devshell-default"]);
            }

            (false, true) => {
                cmd.args(["--security-opt", "label=type:container_runtime_t"]);
            }

            _ => {}
        }

        let username = env::var("USER")
            .map_err(|_| io::Error::other("Failed to get USER environment variable"))?;

        let etc_passwd = EtcPasswd::new(ETC_PASSWD_PATH)?;
        let user_info = etc_passwd
            .iter()
            .find(|user| user.name == username)
            .ok_or_else(|| {
                io::Error::other(format!(
                    "Failed to locate username {username} in {ETC_PASSWD_PATH}"
                ))
            })?;

        let etc_group = EtcGroup::new(ETC_GROUP_PATH)?;
        let primary_group_name = etc_group
            .iter()
            .find(|group| group.id == user_info.group_id)
            .ok_or_else(|| {
                io::Error::other(format!(
                    "Failed to locate primary group id {} in {}",
                    user_info.group_id, ETC_GROUP_PATH
                ))
            })?;

        cmd.args([
            "--env",
            &format!("USERNAME={}", user_info.name),
            "--env",
            &format!("USER_ID={}", user_info.id),
            "--env",
            &format!("GROUP_ID={}", user_info.group_id),
            "--env",
            &format!("GROUP_NAME={}", primary_group_name.name),
        ]);

        Err(PodmanError::IOError(
            cmd.arg(distribution).arg("init").exec(),
        ))
    }

    /// Enters an existing container.
    pub fn enter(&self, container_name: &str) -> Result<(), PodmanError> {
        let container_id = self
            .list()?
            .into_iter()
            .find_map(|container| {
                if container.name_list.contains(&container_name.to_owned()) {
                    Some(container.id)
                } else {
                    None
                }
            })
            .ok_or_else(|| PodmanError::NotFound(container_name.to_owned()))?;

        let err = Command::new("podman")
            .arg("start")
            .args(["--attach", "--interactive"])
            .arg(container_id)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .exec();

        Err(PodmanError::IOError(err))
    }

    /// Deletes an existing container.
    pub fn rm(&self, container_name: &str) -> Result<(), PodmanError> {
        let container_id = self
            .list()?
            .into_iter()
            .find_map(|container| {
                if container.name_list.contains(&container_name.to_owned()) {
                    Some(container.id)
                } else {
                    None
                }
            })
            .ok_or_else(|| PodmanError::NotFound(container_name.to_owned()))?;

        let err = Command::new("podman")
            .arg("rm")
            .arg(container_id)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .exec();

        Err(PodmanError::IOError(err))
    }

    /// Returns the specified string value from the given JSON object.
    fn get_json_object_string(
        json_object: &serde_json::Value,
        key_name: &str,
    ) -> Result<String, PodmanError> {
        json_object
            .get(key_name)
            .ok_or_else(|| PodmanError::MissingJSONKey(key_name.to_owned()))?
            .as_str()
            .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                key_name: key_name.to_owned(),
                expected_type: "string".to_owned(),
            })
            .map(|s| s.to_owned())
    }

    /// Returns the specified object as a string map.
    fn get_json_object_string_map(
        json_object: &serde_json::Value,
        key_name: &str,
    ) -> Result<std::collections::BTreeMap<String, String>, PodmanError> {
        json_object
            .get(key_name)
            .ok_or_else(|| PodmanError::MissingJSONKey(key_name.to_owned()))?
            .as_object()
            .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                key_name: key_name.to_owned(),
                expected_type: "object".to_owned(),
            })
            .map(|object| {
                object
                    .iter()
                    .map(|(key, value)| {
                        let key_string = key.to_owned();
                        let value_string = value
                            .as_str()
                            .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                                key_name: key_name.to_owned(),
                                expected_type: "string".to_owned(),
                            })
                            .map(|string_ref| string_ref.to_owned())?;

                        Ok((key_string, value_string))
                    })
                    .collect::<Result<std::collections::BTreeMap<String, String>, PodmanError>>()
            })?
    }

    /// Returns the specified array as a string vector.
    fn get_json_object_string_list(
        json_object: &serde_json::Value,
        key_name: &str,
    ) -> Result<Vec<String>, PodmanError> {
        json_object
            .get(key_name)
            .ok_or_else(|| PodmanError::MissingJSONKey(key_name.to_owned()))?
            .as_array()
            .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                key_name: key_name.to_owned(),
                expected_type: "string array".to_owned(),
            })?
            .iter()
            .map(|json_value| {
                json_value
                    .as_str()
                    .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                        key_name: key_name.to_owned(),
                        expected_type: "string array item".to_owned(),
                    })
                    .map(|string_ref| string_ref.to_owned())
            })
            .collect()
    }
}
