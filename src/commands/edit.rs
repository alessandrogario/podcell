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
        mount::{Mount, parse_validated_user_mount},
        podman::{Config, Podman, PodmanContainerState},
        port::Port,
    },
};

use clap::Args;

use std::path::PathBuf;

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
fn parse_edit_commands(
    edit_commands: &[String],
) -> Result<Vec<EditOperation>, Box<dyn std::error::Error>> {
    let mut operations = Vec::new();
    let mut tokens = edit_commands.iter();

    while let Some(flag) = tokens.next() {
        let value = tokens
            .next()
            .ok_or_else(|| format!("Missing value after '{flag}' in edit commands"))?;

        let operation = match flag.as_str() {
            "--add-port" => EditOperation::AddPort(value.parse()?),
            "--del-port" => EditOperation::DelPort(value.parse()?),
            "--add-mount" => EditOperation::AddMount(
                parse_validated_user_mount(value).map_err(|error| error.to_string())?,
            ),
            "--del-mount" => EditOperation::DelMount(PathBuf::from(value)),

            _ => return Err(format!("Unknown edit command '{flag}'").into()),
        };

        operations.push(operation);
    }

    Ok(operations)
}

/// Recreates a container with the requested configuration changes.
pub fn run(args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
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
        return Err(format!(
            "The following container is running and can't be edited: {}",
            args.name
        )
        .into());
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
) -> Result<Config, Box<dyn std::error::Error>> {
    for operation in operations {
        match operation {
            EditOperation::AddPort(port) => {
                if config
                    .ports
                    .iter()
                    .any(|existing| existing.host == port.host)
                {
                    return Err(format!(
                        "Host port '{}' is already published on container '{}'",
                        port.host, config.name
                    )
                    .into());
                }

                config.ports.push(port);
            }

            EditOperation::DelPort(host_port) => {
                let position = config
                    .ports
                    .iter()
                    .position(|port| port.host == host_port)
                    .ok_or_else(|| {
                        format!(
                            "Port '{}' is not published on container '{}'",
                            host_port, config.name
                        )
                    })?;

                config.ports.remove(position);
            }

            EditOperation::AddMount(mount) => {
                if config
                    .mounts
                    .iter()
                    .any(|existing| existing.host == mount.host)
                {
                    return Err(format!(
                        "Mount '{}' is already present on container '{}'",
                        mount.host.display(),
                        config.name
                    )
                    .into());
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
                    .ok_or_else(|| {
                        format!(
                            "Mount '{}' does not exist on container '{}'",
                            host_path.display(),
                            config.name
                        )
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
    ) -> Result<Vec<EditOperation>, Box<dyn std::error::Error>> {
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
        assert!(parse_edit_command_args(&["--add-mount", &mount]).is_err());
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
        assert!(parse_edit_command_args(&["--bogus", "value"]).is_err());
    }

    #[test]
    fn flag_without_value_is_rejected() {
        assert!(parse_edit_command_args(&["--add-port"]).is_err());
    }

    #[test]
    fn malformed_port_is_rejected() {
        assert!(parse_edit_command_args(&["--add-port", "notaport"]).is_err());
    }

    #[test]
    fn non_numeric_del_port_is_rejected() {
        assert!(parse_edit_command_args(&["--del-port", "abc"]).is_err());
    }

    #[test]
    fn malformed_mount_is_rejected() {
        assert!(parse_edit_command_args(&["--add-mount", "no-colon"]).is_err());
    }

    #[test]
    fn invalid_mount_mode_is_rejected() {
        let host = temp_host_dir("bad-mode");
        let mount = format!("{}:/m:bogus", host.display());
        assert!(parse_edit_command_args(&["--add-mount", &mount]).is_err());
    }
}
