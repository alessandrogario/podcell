//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::which::{WhichError, which};

use thiserror::Error;

use std::{
    io,
    process::{Command, ExitStatus},
};

/// List of supported package managers
const TOOL_NAME_LIST: &[&str] = &["apt", "dnf", "yum"];

/// Errors produced while detecting or invoking a package manager.
#[derive(Debug, Error)]
pub enum PackageManagerError {
    #[error(transparent)]
    Which(#[from] WhichError),

    #[error("unsupported package manager tool '{0}'")]
    UnsupportedTool(String),

    #[error("no supported package manager tool found in PATH")]
    NotFound,

    #[error("failed to execute package manager command '{command}': {source}")]
    Execute {
        command: String,
        #[source]
        source: io::Error,
    },

    #[error("package manager command failed: {command} (exit_status: {exit_status:?})")]
    CommandFailed {
        command: String,
        exit_status: ExitStatus,
    },
}

#[derive(PartialEq)]
enum ToolType {
    Apt,
    Dnf,
    Yum,
}

pub struct PackageManager {
    tool_type: ToolType,
}

impl PackageManager {
    pub fn new() -> Result<Self, PackageManagerError> {
        Ok(Self {
            tool_type: Self::detect_tool_type()?,
        })
    }

    pub fn update(&self) -> Result<(), PackageManagerError> {
        let args = match self.tool_type {
            ToolType::Apt => vec!["update"],
            ToolType::Dnf | ToolType::Yum => vec!["makecache"],
        };

        self.run_package_manager(&args)
    }

    pub fn install<I, S>(&self, packages: I) -> Result<(), PackageManagerError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut args: Vec<String> = match self.tool_type {
            ToolType::Apt | ToolType::Dnf | ToolType::Yum => {
                vec!["install".to_string(), "-y".to_string()]
            }
        };

        for pkg in packages {
            args.push(pkg.as_ref().to_string());
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_package_manager(&arg_refs)
    }

    fn run_package_manager(&self, args: &[&str]) -> Result<(), PackageManagerError> {
        let command = match self.tool_type {
            ToolType::Apt => "apt-get",
            ToolType::Dnf => "dnf",
            ToolType::Yum => "yum",
        };

        let mut cmd = Command::new(command);
        if ToolType::Apt == self.tool_type {
            cmd.env("DEBIAN_FRONTEND", "noninteractive");
        }

        let command_line = format!("{command} {}", args.join(" "));
        let status = cmd
            .args(args)
            .status()
            .map_err(|source| PackageManagerError::Execute {
                command: command_line.clone(),
                source,
            })?;
        if !status.success() {
            return Err(PackageManagerError::CommandFailed {
                command: command_line,
                exit_status: status,
            });
        }

        Ok(())
    }

    fn detect_tool_type() -> Result<ToolType, PackageManagerError> {
        for &tool_name in TOOL_NAME_LIST {
            if which(tool_name)?.is_some() {
                return match tool_name {
                    "apt" => Ok(ToolType::Apt),
                    "dnf" => Ok(ToolType::Dnf),
                    "yum" => Ok(ToolType::Yum),

                    _ => Err(PackageManagerError::UnsupportedTool(tool_name.to_owned())),
                };
            }
        }

        Err(PackageManagerError::NotFound)
    }
}
