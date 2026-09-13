//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::{
    commands::init::generate_container_base_image,
    utils::{
        mount::{Mount, MountValidationError, parse_validated_user_mount},
        podman::{Config, Podman, PodmanContainerState},
        port::{Port, PortParseError},
    },
};

use {clap::Args, thiserror::Error};

use std::{num::ParseIntError, path::PathBuf};

/// Errors produced while parsing edit operations.
#[derive(Debug, Error)]
pub enum EditParseError {
    #[error("missing value after '{flag}' in edit commands")]
    MissingValue { flag: String },

    #[error("unknown edit command '{flag}'")]
    UnknownCommand { flag: String },

    #[error("invalid value for --add-port: {0}")]
    AddPort(#[source] PortParseError),

    #[error("invalid host port '{value}' for --del-port: {source}")]
    DeletePort {
        value: String,
        #[source]
        source: ParseIntError,
    },

    #[error("invalid value for --add-mount: {0}")]
    AddMount(#[source] MountValidationError),
}

/// Errors produced while applying edit operations to a configuration.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EditApplyError {
    #[error("host port '{port}' is already published on container '{container}'")]
    DuplicatePort { port: u16, container: String },

    #[error("port '{port}' is not published on container '{container}'")]
    PortNotFound { port: u16, container: String },

    #[error("mount '{path}' is already present on container '{container}'")]
    DuplicateMount { path: PathBuf, container: String },

    #[error("mount '{path}' does not exist on container '{container}'")]
    MountNotFound { path: PathBuf, container: String },
}

/// Errors produced by the `edit` command.
#[derive(Debug, Error)]
pub enum EditError {
    #[error(transparent)]
    Podman(#[from] crate::utils::podman::PodmanError),

    #[error(transparent)]
    Parse(#[from] EditParseError),

    #[error(transparent)]
    Apply(#[from] EditApplyError),

    #[error("container '{name}' is running and cannot be edited")]
    ContainerRunning { name: String },

    #[error(transparent)]
    GenerateImage(#[from] crate::commands::init::GenerateContainerBaseImageError),
}

/// A configuration change applied during container recreation.
enum EditOperation {
    /// Adds a published port.
    AddPort(Port),
    /// Removes a published host port.
    DelPort(u16),
    /// Adds a bind mount.
    AddMount(Mount),
    /// Removes a bind mount by host path.
    DelMount(PathBuf),
}

/// Help text describing the supported edit operations.
const EDIT_COMMANDS_HELP: &str = r#"A repeatable list of the following edit commands:

  --add-port host:container/protocol
  --del-port host

Protocol can either be `tcp` or `udp`

  --add-mount host:container:mode
  --del-mount host

Mode can either be `rw` or `ro`"#;

/// Edit an existing container.
#[derive(Args)]
pub struct Arguments {
    /// Container name.
    #[arg()]
    name: String,

    /// A repeatable list of edit commands (see `--help`).
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        num_args = 1..,
        help = EDIT_COMMANDS_HELP
    )]
    edit_commands: Vec<String>,
}

/// Parses trailing edit arguments into ordered operations.
fn parse_edit_commands(edit_commands: &[String]) -> Result<Vec<EditOperation>, EditParseError> {
    let mut operations = Vec::new();
    let mut tokens = edit_commands.iter();

    while let Some(flag) = tokens.next() {
        let value = tokens
            .next()
            .ok_or_else(|| EditParseError::MissingValue { flag: flag.clone() })?;

        let operation = match flag.as_str() {
            "--add-port" => EditOperation::AddPort(value.parse().map_err(EditParseError::AddPort)?),
            "--del-port" => EditOperation::DelPort(value.parse().map_err(|source| {
                EditParseError::DeletePort {
                    value: value.clone(),
                    source,
                }
            })?),
            "--add-mount" => EditOperation::AddMount(
                parse_validated_user_mount(value).map_err(EditParseError::AddMount)?,
            ),
            "--del-mount" => EditOperation::DelMount(PathBuf::from(value)),

            _ => return Err(EditParseError::UnknownCommand { flag: flag.clone() }),
        };

        operations.push(operation);
    }

    Ok(operations)
}

/// Recreates a container with the requested configuration changes.
pub fn run(args: Arguments) -> Result<(), EditError> {
    if args.edit_commands.is_empty() {
        eprintln!("Nothing to do!");
        return Ok(());
    }

    let podman = Podman::new();
    if podman
        .list()?
        .iter()
        .filter(|&container| {
            matches!(
                container.state,
                PodmanContainerState::Running
                    | PodmanContainerState::Paused
                    | PodmanContainerState::Stopping
                    | PodmanContainerState::Restarting
            )
        })
        .any(|container| container.name_list.contains(&args.name))
    {
        return Err(EditError::ContainerRunning { name: args.name });
    }

    let config = podman.inspect(&args.name)?;
    let operations = parse_edit_commands(&args.edit_commands)?;

    let mut config = process_edit_args(config, operations)?;
    config.image_ref = format!("localhost/podcell:{}", config.name);

    generate_container_base_image(&podman, &config)?;

    podman.rm_by_id(&config.name)?;
    podman.create(&config)?;

    Ok(())
}

/// Applies validated edit operations to a container configuration.
fn process_edit_args(
    mut config: Config,
    operations: Vec<EditOperation>,
) -> Result<Config, EditApplyError> {
    for operation in operations {
        match operation {
            EditOperation::AddPort(port) => {
                if config
                    .ports
                    .iter()
                    .any(|existing| existing.host == port.host)
                {
                    return Err(EditApplyError::DuplicatePort {
                        port: port.host,
                        container: config.name,
                    });
                }

                config.ports.push(port);
            }

            EditOperation::DelPort(host_port) => {
                let position = config
                    .ports
                    .iter()
                    .position(|port| port.host == host_port)
                    .ok_or_else(|| EditApplyError::PortNotFound {
                        port: host_port,
                        container: config.name.clone(),
                    })?;

                config.ports.remove(position);
            }

            EditOperation::AddMount(mount) => {
                if config
                    .mounts
                    .iter()
                    .any(|existing| existing.host == mount.host)
                {
                    return Err(EditApplyError::DuplicateMount {
                        path: mount.host,
                        container: config.name,
                    });
                }

                config.mounts.push(mount);
            }

            EditOperation::DelMount(host_path) => {
                let host_path = host_path
                    .canonicalize()
                    .unwrap_or_else(|_| host_path.clone());

                let position = config
                    .mounts
                    .iter()
                    .position(|mount| mount.host == host_path)
                    .ok_or_else(|| EditApplyError::MountNotFound {
                        path: host_path.clone(),
                        container: config.name.clone(),
                    })?;

                config.mounts.remove(position);
            }
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{mount::MountMode, port::PortProtocol};

    use std::path::Path;

    fn temp_host_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("podcell-parse-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn parse_edit_command_args(
        edit_commands: &[&str],
    ) -> Result<Vec<EditOperation>, EditParseError> {
        let edit_commands = edit_commands
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>();
        parse_edit_commands(&edit_commands)
    }

    #[test]
    fn empty_input_yields_no_operations() {
        assert!(parse_edit_command_args(&[]).unwrap().is_empty());
    }

    #[test]
    fn add_port_defaults_protocol_to_tcp() {
        let ops = parse_edit_command_args(&["--add-port", "8080:80"]).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(
            &ops[0],
            EditOperation::AddPort(port)
                if port.host == 8080 && port.container == 80 && port.protocol == PortProtocol::Tcp
        ));
    }

    #[test]
    fn add_port_accepts_explicit_udp_protocol() {
        let ops = parse_edit_command_args(&["--add-port", "5353:53/udp"]).unwrap();
        assert!(matches!(
            &ops[0],
            EditOperation::AddPort(port) if port.protocol == PortProtocol::Udp
        ));
    }

    #[test]
    fn del_port_parses_host_port() {
        let ops = parse_edit_command_args(&["--del-port", "9090"]).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], EditOperation::DelPort(9090)));
    }

    #[test]
    fn add_mount_parses_host_container_and_mode() {
        let host = temp_host_dir("add-mount");
        let mount = format!("{}:/mnt/data:rw", host.display());
        let ops = parse_edit_command_args(&["--add-mount", &mount]).unwrap();
        assert!(matches!(
            &ops[0],
            EditOperation::AddMount(mount)
                if mount.host == host.canonicalize().unwrap()
                    && mount.container == Path::new("/mnt/data")
                    && mount.mode == MountMode::Rw
        ));
    }

    #[test]
    fn add_mount_defaults_mode_to_ro() {
        let host = temp_host_dir("add-mount-ro");
        let mount = format!("{}:/mnt", host.display());
        let ops = parse_edit_command_args(&["--add-mount", &mount]).unwrap();
        assert!(matches!(
            &ops[0],
            EditOperation::AddMount(mount) if mount.mode == MountMode::Ro
        ));
    }

    #[test]
    fn add_mount_canonicalizes_relative_host_path() {
        let ops = parse_edit_command_args(&["--add-mount", ".:/mnt"]).unwrap();
        assert!(matches!(
            &ops[0],
            EditOperation::AddMount(mount)
                if mount.host == std::env::current_dir().unwrap().canonicalize().unwrap()
        ));
    }

    #[test]
    fn add_mount_rejects_missing_host_path() {
        let missing =
            std::env::temp_dir().join(format!("podcell-missing-mount-{}", std::process::id()));
        let mount = format!("{}:/mnt", missing.display());
        assert!(matches!(
            parse_edit_command_args(&["--add-mount", &mount]),
            Err(EditParseError::AddMount(
                MountValidationError::HostPathMissing { path }
            )) if path == missing
        ));
    }

    #[test]
    fn add_mount_preserves_spaces_in_host_path() {
        let host = temp_host_dir("host path with spaces");
        let mount = format!("{}:/mnt", host.display());
        let ops = parse_edit_command_args(&["--add-mount", &mount]).unwrap();
        assert!(matches!(
            &ops[0],
            EditOperation::AddMount(mount) if mount.host == host.canonicalize().unwrap()
        ));
    }

    #[test]
    fn del_mount_parses_host_path() {
        let ops = parse_edit_command_args(&["--del-mount", "/some/path"]).unwrap();
        assert!(matches!(
            &ops[0],
            EditOperation::DelMount(path) if *path == Path::new("/some/path")
        ));
    }

    #[test]
    fn preserves_command_order() {
        let host = temp_host_dir("order");
        let mount = format!("{}:/m:ro", host.display());
        let ops = parse_edit_command_args(&[
            "--del-mount",
            "/old",
            "--add-port",
            "8080:80",
            "--del-port",
            "7777",
            "--add-mount",
            &mount,
        ])
        .unwrap();
        assert_eq!(ops.len(), 4);
        assert!(matches!(&ops[0], EditOperation::DelMount(_)));
        assert!(matches!(&ops[1], EditOperation::AddPort(_)));
        assert!(matches!(&ops[2], EditOperation::DelPort(7777)));
        assert!(matches!(&ops[3], EditOperation::AddMount(_)));
    }

    #[test]
    fn repeated_flag_appends_in_order() {
        let first = temp_host_dir("repeat-1");
        let second = temp_host_dir("repeat-2");
        let first_mount = format!("{}:/a:ro", first.display());
        let second_mount = format!("{}:/b:rw", second.display());
        let ops =
            parse_edit_command_args(&["--add-mount", &first_mount, "--add-mount", &second_mount])
                .unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(
            &ops[0],
            EditOperation::AddMount(mount) if mount.container == Path::new("/a")
        ));
        assert!(matches!(
            &ops[1],
            EditOperation::AddMount(mount) if mount.container == Path::new("/b")
        ));
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(matches!(
            parse_edit_command_args(&["--bogus", "value"]),
            Err(EditParseError::UnknownCommand { flag }) if flag == "--bogus"
        ));
    }

    #[test]
    fn flag_without_value_is_rejected() {
        assert!(matches!(
            parse_edit_command_args(&["--add-port"]),
            Err(EditParseError::MissingValue { flag }) if flag == "--add-port"
        ));
    }

    #[test]
    fn malformed_port_is_rejected() {
        assert!(matches!(
            parse_edit_command_args(&["--add-port", "notaport"]),
            Err(EditParseError::AddPort(PortParseError::BadShape(input)))
                if input == "notaport"
        ));
    }

    #[test]
    fn non_numeric_del_port_is_rejected() {
        assert!(matches!(
            parse_edit_command_args(&["--del-port", "abc"]),
            Err(EditParseError::DeletePort { value, .. }) if value == "abc"
        ));
    }

    #[test]
    fn malformed_mount_is_rejected() {
        assert!(matches!(
            parse_edit_command_args(&["--add-mount", "no-colon"]),
            Err(EditParseError::AddMount(MountValidationError::Parse(
                crate::utils::mount::MountParseError::BadShape(input)
            ))) if input == "no-colon"
        ));
    }

    #[test]
    fn invalid_mount_mode_is_rejected() {
        let host = temp_host_dir("bad-mode");
        let mount = format!("{}:/m:bogus", host.display());
        assert!(matches!(
            parse_edit_command_args(&["--add-mount", &mount]),
            Err(EditParseError::AddMount(MountValidationError::Parse(
                crate::utils::mount::MountParseError::BadMode { got, .. }
            ))) if got == "bogus"
        ));
    }

    #[test]
    fn duplicate_port_reports_structured_apply_error() {
        let config = Config {
            image_ref: "fedora:42".to_owned(),
            name: "dev".to_owned(),
            mounts: Vec::new(),
            ports: vec!["8080:80".parse().unwrap()],
        };

        let error = process_edit_args(
            config,
            vec![EditOperation::AddPort("8080:8080".parse().unwrap())],
        )
        .unwrap_err();

        assert_eq!(
            error,
            EditApplyError::DuplicatePort {
                port: 8080,
                container: "dev".to_owned(),
            }
        );
    }

    #[test]
    fn missing_mount_reports_structured_apply_error() {
        let config = Config {
            image_ref: "fedora:42".to_owned(),
            name: "dev".to_owned(),
            mounts: Vec::new(),
            ports: Vec::new(),
        };

        let path = PathBuf::from("/missing");
        let error =
            process_edit_args(config, vec![EditOperation::DelMount(path.clone())]).unwrap_err();

        assert_eq!(
            error,
            EditApplyError::MountNotFound {
                path,
                container: "dev".to_owned(),
            }
        );
    }
}
