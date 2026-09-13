//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::{
    group::{EtcGroup, EtcGroupError},
    mount::{Mount, MountMode, MountRenderError},
    passwd::{EtcPasswd, EtcPasswdError},
    port::{Port, PortParseError},
};

use thiserror::Error;

use std::{
    env, io,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    string::FromUtf8Error,
};

/// The name of the Podman executable.
const PODMAN_EXECUTABLE_NAME: &str = "podman";

/// The path within the container where our own binary is mounted.
const PODCELL_MOUNT_PATH: &str = "/usr/bin/podcell";

/// Path to the /etc/passwd file.
const ETC_PASSWD_PATH: &str = "/etc/passwd";

/// Path to the /etc/group file.
const ETC_GROUP_PATH: &str = "/etc/group";

/// Errors returned by Podman operations and output parsing.
#[derive(Error, Debug)]
pub enum PodmanError {
    /// Podman could not be executed.
    #[error("failed to execute podman while attempting to {operation}: {source}")]
    Execute {
        operation: &'static str,
        #[source]
        source: io::Error,
    },

    /// The current podcell executable could not be located.
    #[error("failed to locate the current podcell executable: {0}")]
    CurrentExecutable(#[source] io::Error),

    /// Podman output was not valid UTF-8.
    #[error("podman has returned invalid UTF-8 output")]
    InvalidOutputEncoding(#[from] FromUtf8Error),

    /// Podman output was not valid JSON.
    #[error("podman has returned invalid JSON output")]
    InvalidJSONOutput(#[from] serde_json::Error),

    /// A required key was absent from Podman JSON output.
    #[error("the JSON returned by podman is missing a required field: {0}")]
    MissingJSONKey(String),

    /// A Podman JSON field had an unexpected type.
    #[error(
        "podman returned an invalid JSON field (key: {key_name:?}, expected type: {expected_type:?})"
    )]
    InvalidJSONKeyType {
        /// The field containing the invalid value.
        key_name: String,
        /// The expected JSON type or format.
        expected_type: String,
    },

    /// Podman returned an unknown container state.
    #[error(transparent)]
    InvalidContainerState(#[from] PodmanContainerStateParseError),

    /// The requested podcell-managed container was not found.
    #[error("the following container is either missing or is not managed by podcell: {0}")]
    NotFound(String),

    /// A Podman command returned a failure status.
    #[error("podman failed to {operation} (exit_status: {exit_status:?})")]
    CommandError {
        operation: &'static str,
        exit_status: ExitStatus,
    },

    /// A path passed to Podman was not valid UTF-8.
    #[error("{description} path '{path}' is not valid UTF-8")]
    NonUtf8Path {
        description: &'static str,
        path: String,
    },

    /// A required environment variable could not be read.
    #[error("failed to access the {name} environment variable: {source}")]
    EnvironmentVariable {
        name: &'static str,
        #[source]
        source: env::VarError,
    },

    /// The current user was absent from the passwd database.
    #[error("failed to locate username '{username}' in {ETC_PASSWD_PATH}")]
    UserNotFound { username: String },

    /// The current user's primary group was absent from the group database.
    #[error("failed to locate primary group id {group_id} in {ETC_GROUP_PATH}")]
    PrimaryGroupNotFound { group_id: u32 },

    /// Podman's group file could not be read or parsed.
    #[error("group file error")]
    EtcGroupError(#[from] EtcGroupError),

    /// Podman's passwd file could not be read or parsed.
    #[error("passwd file error")]
    EtcPasswdError(#[from] EtcPasswdError),

    /// A mount could not be rendered for Podman.
    #[error("failed to render mount as a podman --volume argument")]
    MountRenderError(#[from] MountRenderError),

    /// A port binding from Podman could not be parsed.
    #[error("invalid port binding in podman inspect output")]
    PortParseError(#[from] PortParseError),
}

/// These states correspond to the lifecycle phases of a container as reported by Podman.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl PodmanContainerState {
    /// Lowercase string representation, matching the strings podman emits in `ps --format=json`.
    pub fn as_str(self) -> &'static str {
        match self {
            PodmanContainerState::Created => "created",
            PodmanContainerState::Running => "running",
            PodmanContainerState::Paused => "paused",
            PodmanContainerState::Exited => "exited",
            PodmanContainerState::Stopped => "stopped",
            PodmanContainerState::Stopping => "stopping",
            PodmanContainerState::Restarting => "restarting",
            PodmanContainerState::Dead => "dead",
        }
    }
}

impl std::fmt::Display for PodmanContainerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PodmanContainerState {
    type Err = PodmanContainerStateParseError;

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
            _ => Err(PodmanContainerStateParseError {
                state: s.to_owned(),
            }),
        }
    }
}

/// Error returned when Podman reports an unknown container state.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid container state '{state}'")]
pub struct PodmanContainerStateParseError {
    pub state: String,
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

/// Container settings used by create and edit operations.
#[derive(Debug, Clone)]
pub struct Config {
    /// Source image reference.
    pub image_ref: String,
    /// Container name.
    pub name: String,
    /// Bind mounts exposed to the container.
    pub mounts: Vec<Mount>,
    /// Ports published by the container.
    pub ports: Vec<Port>,
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
            .output()
            .map_err(|source| PodmanError::Execute {
                operation: "list containers",
                source,
            })?;

        if !podman_output.status.success() {
            return Err(PodmanError::CommandError {
                operation: "list containers",
                exit_status: podman_output.status,
            });
        }

        let json_output: serde_json::Value =
            serde_json::from_str(&String::from_utf8(podman_output.stdout)?)?;

        let mut container_list = Vec::new();

        for json_object in
            json_output
                .as_array()
                .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                    key_name: "(root)".to_owned(),
                    expected_type: "array".to_owned(),
                })?
        {
            let label_map = Self::get_json_object_string_map(json_object, "Labels")?;
            if !label_map
                .iter()
                .any(|(key, value)| key == "manager" && value == "podcell")
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
                state: string_state.parse()?,
            });
        }

        Ok(container_list)
    }

    /// Creates a container from `config`.
    pub fn create(&self, config: &Config) -> Result<(), PodmanError> {
        let mut cmd = Command::new(PODMAN_EXECUTABLE_NAME);

        cmd.arg("run")
            .arg("--init")
            .arg("--tty")
            .arg("--interactive")
            .args(["--label", "manager=podcell"])
            .args(["--hosts-file", "image"])
            .arg(format!("--add-host={}:127.0.0.1", config.name))
            .arg(format!("--add-host={}:::1", config.name));

        cmd.arg("--network=pasta");

        cmd.args(["--name", &config.name])
            .args(["--hostname", &config.name])
            .arg("--userns=keep-id")
            .arg("--user=0:0")
            .args(["--security-opt", "mask=/proc/acpi,/proc/kcore,/proc/keys,/proc/sched_debug,/proc/timer_list,/proc/timer_stats,/sys/firmware"])
            .arg("--cap-drop=AUDIT_CONTROL,AUDIT_READ,AUDIT_WRITE,BPF,BLOCK_SUSPEND,CHECKPOINT_RESTORE,IPC_LOCK,IPC_OWNER,KILL,LEASE,LINUX_IMMUTABLE,MAC_ADMIN,MAC_OVERRIDE,MKNOD,NET_ADMIN,NET_BROADCAST,PERFMON,SETFCAP,SETPCAP,SYSLOG,SYS_ADMIN,SYS_BOOT,SYS_MODULE,SYS_NICE,SYS_PACCT,SYS_PTRACE,SYS_RAWIO,SYS_RESOURCE,SYS_TIME,SYS_TTY_CONFIG,WAKE_ALARM")
            .arg("--cap-add=DAC_OVERRIDE,DAC_READ_SEARCH");

        let self_path = env::current_exe().map_err(PodmanError::CurrentExecutable)?;
        let self_path_str = self_path.to_str().ok_or_else(|| PodmanError::NonUtf8Path {
            description: "podcell executable",
            path: self_path.display().to_string(),
        })?;

        cmd.args([
            "--volume",
            &format!("{self_path_str}:{PODCELL_MOUNT_PATH}:ro,z"),
        ])
        .args(["--entrypoint", PODCELL_MOUNT_PATH]);

        for mount in &config.mounts {
            let volume = mount.to_podman_volume_arg()?;
            cmd.args(["--volume", &volume]);
        }

        for port in &config.ports {
            let publish = port.to_podman_publish_arg();
            cmd.args(["--publish", &publish]);
        }

        if std::path::Path::new("/sys/fs/selinux/enforce").exists() {
            cmd.args(["--security-opt", "label=type:container_runtime_t"]);
        }

        let username = env::var("USER").map_err(|source| PodmanError::EnvironmentVariable {
            name: "USER",
            source,
        })?;

        let etc_passwd = EtcPasswd::new(ETC_PASSWD_PATH)?;
        let user_info = etc_passwd
            .iter()
            .find(|user| user.name == username)
            .ok_or_else(|| PodmanError::UserNotFound {
                username: username.clone(),
            })?;

        let etc_group = EtcGroup::new(ETC_GROUP_PATH)?;
        let primary_group_name = etc_group
            .iter()
            .find(|group| group.id == user_info.group_id)
            .ok_or(PodmanError::PrimaryGroupNotFound {
                group_id: user_info.group_id,
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

        cmd.arg(&config.image_ref).arg("init");

        let status = cmd.status().map_err(|source| PodmanError::Execute {
            operation: "create container",
            source,
        })?;
        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "create container",
                exit_status: status,
            });
        }

        Ok(())
    }

    /// Looks up a podcell-managed container by name. Returns the full container record
    /// (id + state + image info) so callers can make state decisions without re-querying.
    pub fn find_by_name(&self, container_name: &str) -> Result<PodmanContainer, PodmanError> {
        self.list()?
            .into_iter()
            .find(|container| container.name_list.iter().any(|n| n == container_name))
            .ok_or_else(|| PodmanError::NotFound(container_name.to_owned()))
    }

    /// Starts an existing container without attaching. Blocks until the container is up.
    ///
    /// Returns rather than `exec`-replacing because the caller may want to print
    /// status afterwards (e.g. an "already running" notice or post-start diagnostics).
    pub fn start(&self, container_id: &str) -> Result<(), PodmanError> {
        let status = Command::new(PODMAN_EXECUTABLE_NAME)
            .arg("start")
            .arg(container_id)
            .status()
            .map_err(|source| PodmanError::Execute {
                operation: "start container",
                source,
            })?;

        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "start container",
                exit_status: status,
            });
        }

        Ok(())
    }

    /// Stops a running container. Blocks until SIGTERM-then-SIGKILL completes.
    ///
    /// Returns rather than `exec`-replacing because the caller may want to print
    /// status afterwards (e.g. an "already stopped" notice or post-stop diagnostics).
    pub fn stop(&self, container_id: &str) -> Result<(), PodmanError> {
        let status = Command::new(PODMAN_EXECUTABLE_NAME)
            .arg("stop")
            .arg(container_id)
            .status()
            .map_err(|source| PodmanError::Execute {
                operation: "stop container",
                source,
            })?;

        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "stop container",
                exit_status: status,
            });
        }

        Ok(())
    }

    /// Execs a command inside a running container, waiting for it to finish.
    pub fn exec(&self, container_id: &str, command: &[&str]) -> Result<(), PodmanError> {
        let status = Command::new(PODMAN_EXECUTABLE_NAME)
            .arg("exec")
            .arg(container_id)
            .args(command)
            .status()
            .map_err(|source| PodmanError::Execute {
                operation: "execute container command",
                source,
            })?;

        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "execute container command",
                exit_status: status,
            });
        }

        Ok(())
    }

    /// Copies `source` on the host into `dest` inside the container.
    pub fn cp(
        &self,
        container_id: &str,
        source: &std::path::Path,
        dest: &str,
    ) -> Result<(), PodmanError> {
        let source_str = source.to_str().ok_or_else(|| PodmanError::NonUtf8Path {
            description: "source",
            path: source.display().to_string(),
        })?;

        let status = Command::new(PODMAN_EXECUTABLE_NAME)
            .args(["cp", source_str, &format!("{container_id}:{dest}")])
            .status()
            .map_err(|source| PodmanError::Execute {
                operation: "copy into container",
                source,
            })?;

        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "copy into container",
                exit_status: status,
            });
        }

        Ok(())
    }

    /// Execs a command inside a running container, replacing the current process.
    /// Always allocates a TTY and connects stdin so the user can interact with the command.
    pub fn exec_interactive(
        &self,
        container_id: &str,
        command: &[&str],
    ) -> Result<(), PodmanError> {
        let err = Command::new(PODMAN_EXECUTABLE_NAME)
            .arg("exec")
            .arg("--tty")
            .arg("--interactive")
            .arg(container_id)
            .args(command)
            .exec();

        Err(PodmanError::Execute {
            operation: "execute interactive container command",
            source: err,
        })
    }

    /// Deletes a container by id. Replaces the current process with `podman rm`.
    pub fn rm_by_id(&self, container_id: &str) -> Result<(), PodmanError> {
        let status = Command::new(PODMAN_EXECUTABLE_NAME)
            .arg("rm")
            .arg(container_id)
            .status()
            .map_err(|source| PodmanError::Execute {
                operation: "remove container",
                source,
            })?;

        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "remove container",
                exit_status: status,
            });
        }

        Ok(())
    }

    /// Commits a container to an image.
    pub fn commit(&self, container_id: &str, image_ref: &str) -> Result<(), PodmanError> {
        let status = Command::new(PODMAN_EXECUTABLE_NAME)
            .args(["commit", container_id, image_ref])
            .status()
            .map_err(|source| PodmanError::Execute {
                operation: "commit container",
                source,
            })?;

        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "commit container",
                exit_status: status,
            });
        }

        Ok(())
    }

    /// Builds and tags an image from a build context.
    pub fn build(&self, context_dir: &Path, image_ref: &str) -> Result<(), PodmanError> {
        let status = Command::new(PODMAN_EXECUTABLE_NAME)
            .arg("build")
            .arg("--tag")
            .arg(image_ref)
            .arg(context_dir)
            .status()
            .map_err(|source| PodmanError::Execute {
                operation: "build image",
                source,
            })?;

        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "build image",
                exit_status: status,
            });
        }

        Ok(())
    }

    /// Removes an image.
    pub fn rmi(&self, image_ref: &str) -> Result<(), PodmanError> {
        let status = Command::new(PODMAN_EXECUTABLE_NAME)
            .arg("rmi")
            .arg(image_ref)
            .status()
            .map_err(|source| PodmanError::Execute {
                operation: "remove image",
                source,
            })?;

        if !status.success() {
            return Err(PodmanError::CommandError {
                operation: "remove image",
                exit_status: status,
            });
        }

        Ok(())
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

    /// Reads a container's editable configuration.
    pub fn inspect(&self, container_id: &str) -> Result<Config, PodmanError> {
        let output = Command::new(PODMAN_EXECUTABLE_NAME)
            .args(["inspect", "--type=container", container_id])
            .stderr(std::process::Stdio::inherit())
            .output()
            .map_err(|source| PodmanError::Execute {
                operation: "inspect container",
                source,
            })?;

        if !output.status.success() {
            return Err(PodmanError::CommandError {
                operation: "inspect container",
                exit_status: output.status,
            });
        }

        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(output.stdout)?)?;

        let containers = json
            .as_array()
            .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                key_name: "(root)".to_owned(),
                expected_type: "array".to_owned(),
            })?;

        let container = containers
            .first()
            .ok_or_else(|| PodmanError::NotFound(container_id.to_owned()))?;

        let image_ref = Self::get_json_object_string(container, "ImageName")?;
        let name = Self::get_json_object_string(container, "Name")?;
        let mounts = Self::get_container_bind_mounts(container)?;
        let ports = Self::get_container_ports(container)?;

        Ok(Config {
            image_ref,
            name: name.trim_start_matches('/').to_owned(),
            mounts,
            ports,
        })
    }

    /// Extracts all published port bindings from inspect output.
    fn get_container_ports(container: &serde_json::Value) -> Result<Vec<Port>, PodmanError> {
        let mut ports = Vec::new();

        if let Some(port_bindings) = container
            .get("HostConfig")
            .and_then(|host_config| host_config.get("PortBindings"))
            .and_then(|port_bindings| port_bindings.as_object())
        {
            for (key, bindings) in port_bindings {
                let (container_port, protocol) =
                    key.split_once('/')
                        .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                            key_name: key.to_owned(),
                            expected_type: "CONTAINER_PORT/PROTOCOL".to_owned(),
                        })?;

                let bindings =
                    bindings
                        .as_array()
                        .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                            key_name: key.to_owned(),
                            expected_type: "binding array".to_owned(),
                        })?;

                for binding in bindings {
                    let host_port = Self::get_json_object_string(binding, "HostPort")?;
                    let publish = format!("{host_port}:{container_port}/{protocol}");
                    ports.push(publish.parse::<Port>().map_err(PodmanError::from)?);
                }
            }
        }

        Ok(ports)
    }

    /// Extracts user bind mounts from inspect output.
    fn get_container_bind_mounts(container: &serde_json::Value) -> Result<Vec<Mount>, PodmanError> {
        let mounts = container
            .get("Mounts")
            .ok_or_else(|| PodmanError::MissingJSONKey("Mounts".to_owned()))?
            .as_array()
            .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                key_name: "Mounts".to_owned(),
                expected_type: "array".to_owned(),
            })?;

        let mut user_mounts = Vec::new();
        for mount in mounts {
            let mount_type = Self::get_json_object_string(mount, "Type")?;
            if mount_type != "bind" {
                continue;
            }

            let destination = Self::get_json_object_string(mount, "Destination")?;
            if destination == PODCELL_MOUNT_PATH {
                continue;
            }

            let source = Self::get_json_object_string(mount, "Source")?;
            let read_write = mount
                .get("RW")
                .and_then(|read_write| read_write.as_bool())
                .ok_or_else(|| PodmanError::InvalidJSONKeyType {
                    key_name: "RW".to_owned(),
                    expected_type: "boolean".to_owned(),
                })?;

            user_mounts.push(Mount {
                host: PathBuf::from(source),
                container: PathBuf::from(destination),
                mode: if read_write {
                    MountMode::Rw
                } else {
                    MountMode::Ro
                },
            });
        }

        Ok(user_mounts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::port::PortProtocol;

    #[test]
    fn unknown_container_state_returns_the_original_value() {
        let error = "migrating".parse::<PodmanContainerState>().unwrap_err();

        assert_eq!(
            error,
            PodmanContainerStateParseError {
                state: "migrating".to_owned(),
            }
        );
    }

    #[test]
    fn get_container_ports_preserves_multiple_host_ports_for_one_container_port() {
        let container = serde_json::json!({
            "HostConfig": {
                "PortBindings": {
                    "80/tcp": [
                        { "HostIp": "0.0.0.0", "HostPort": "8080" },
                        { "HostIp": "0.0.0.0", "HostPort": "8081" }
                    ]
                }
            }
        });

        let ports = Podman::get_container_ports(&container).unwrap();

        assert_eq!(ports.len(), 2);
        assert!(ports.contains(&Port {
            host: 8080,
            container: 80,
            protocol: PortProtocol::Tcp,
        }));
        assert!(ports.contains(&Port {
            host: 8081,
            container: 80,
            protocol: PortProtocol::Tcp,
        }));
    }

    #[test]
    fn get_container_ports_preserves_the_parse_error_variant() {
        let container = serde_json::json!({
            "HostConfig": {
                "PortBindings": {
                    "invalid/tcp": [{ "HostPort": "8080" }]
                }
            }
        });

        assert!(matches!(
            Podman::get_container_ports(&container),
            Err(PodmanError::PortParseError(PortParseError::InvalidPort {
                field: "container",
                value,
                ..
            })) if value == "invalid"
        ));
    }
}
