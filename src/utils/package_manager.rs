//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::which::which;

use std::{io, process::Command};

const TOOL_NAME_LIST: &[&str] = &["apt", "dnf", "yum"];

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
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            tool_type: Self::detect_tool_type()?,
        })
    }

    pub fn update(&self) -> io::Result<()> {
        let args = match self.tool_type {
            ToolType::Apt => vec!["update"],
            ToolType::Dnf | ToolType::Yum => vec!["makecache"],
        };

        self.run_package_manager(&args)
    }

    pub fn install<I, S>(&self, packages: I) -> io::Result<()>
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

    fn run_package_manager(&self, args: &[&str]) -> io::Result<()> {
        let command = match self.tool_type {
            ToolType::Apt => "apt-get",
            ToolType::Dnf => "dnf",
            ToolType::Yum => "yum",
        };

        let mut cmd = Command::new(command);
        if ToolType::Apt == self.tool_type {
            cmd.env("DEBIAN_FRONTEND", "noninteractive");
        }

        let status = cmd.args(args).status()?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "Failed to run the package manager: {command} {args:?}",
            )));
        }

        Ok(())
    }

    fn detect_tool_type() -> io::Result<ToolType> {
        for &tool_name in TOOL_NAME_LIST {
            if which(tool_name)?.is_some() {
                return match tool_name {
                    "apt" => Ok(ToolType::Apt),
                    "dnf" => Ok(ToolType::Dnf),
                    "yum" => Ok(ToolType::Yum),

                    _ => Err(io::Error::other("Unknown package manager tool")),
                };
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No supported package manager tool found in PATH",
        ))
    }
}
