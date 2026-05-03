//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use std::{path::PathBuf, str::FromStr};

use thiserror::Error;

/// Access mode for a bind mount inside the container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountMode {
    Ro,
    Rw,
}

impl MountMode {
    pub fn as_str(self) -> &'static str {
        match self {
            MountMode::Ro => "ro",
            MountMode::Rw => "rw",
        }
    }
}

/// Errors produced when parsing a `--mount HOST:CONTAINER[:MODE]` string.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MountParseError {
    #[error("invalid mount '{0}': expected HOST:CONTAINER[:MODE]")]
    BadShape(String),

    #[error("invalid mount '{input}': {field} path is empty")]
    EmptyPath { input: String, field: &'static str },

    #[error("invalid mount '{input}': {field} path '{path}' must be absolute")]
    RelativePath {
        input: String,
        field: &'static str,
        path: String,
    },

    #[error("invalid mount '{input}': mode must be 'ro' or 'rw', got '{got}'")]
    BadMode { input: String, got: String },
}

/// Errors produced when rendering a `Mount` to a podman `--volume` argument.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MountRenderError {
    #[error("{field} path is not valid UTF-8: {path}")]
    NonUtf8Path { field: &'static str, path: String },

    #[error(
        "{field} path '{path}' contains ':', which would break podman's --volume parsing. \
         This usually indicates a symlink resolved to a path with a colon in it."
    )]
    PathContainsColon { field: &'static str, path: String },
}

/// A user-supplied bind mount, parsed from a `--mount` argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub host: PathBuf,
    pub container: PathBuf,
    pub mode: MountMode,
}

impl Mount {
    /// Render this mount as the value of a podman `--volume` argument.
    ///
    /// Errors if either path is not valid UTF-8 or contains a literal `:` (which
    /// would be misparsed by podman as a volume-arg separator).
    pub fn to_volume_arg(&self) -> Result<String, MountRenderError> {
        let host = path_as_str(&self.host, "host")?;
        let container = path_as_str(&self.container, "container")?;
        Ok(format!("{host}:{container}:{},z", self.mode.as_str()))
    }
}

fn path_as_str<'a>(
    path: &'a std::path::Path,
    field: &'static str,
) -> Result<&'a str, MountRenderError> {
    let s = path.to_str().ok_or_else(|| MountRenderError::NonUtf8Path {
        field,
        path: path.display().to_string(),
    })?;
    if s.contains(':') {
        return Err(MountRenderError::PathContainsColon {
            field,
            path: s.to_owned(),
        });
    }
    Ok(s)
}

impl FromStr for Mount {
    type Err = MountParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();

        let (host_str, container_str, mode) = match parts.as_slice() {
            [host, container] => (*host, *container, MountMode::Ro),
            [host, container, "ro"] => (*host, *container, MountMode::Ro),
            [host, container, "rw"] => (*host, *container, MountMode::Rw),
            [_, _, mode] => {
                return Err(MountParseError::BadMode {
                    input: s.to_owned(),
                    got: (*mode).to_owned(),
                });
            }
            _ => return Err(MountParseError::BadShape(s.to_owned())),
        };

        if host_str.is_empty() {
            return Err(MountParseError::EmptyPath {
                input: s.to_owned(),
                field: "host",
            });
        }
        if container_str.is_empty() {
            return Err(MountParseError::EmptyPath {
                input: s.to_owned(),
                field: "container",
            });
        }

        let host = PathBuf::from(host_str);
        let container = PathBuf::from(container_str);

        if !host.is_absolute() {
            return Err(MountParseError::RelativePath {
                input: s.to_owned(),
                field: "host",
                path: host_str.to_owned(),
            });
        }
        if !container.is_absolute() {
            return Err(MountParseError::RelativePath {
                input: s.to_owned(),
                field: "container",
                path: container_str.to_owned(),
            });
        }

        Ok(Mount {
            host,
            container,
            mode,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Mount, MountParseError> {
        s.parse()
    }

    #[test]
    fn two_part_defaults_to_ro() {
        let m = parse("/foo:/bar").unwrap();
        assert_eq!(m.host, PathBuf::from("/foo"));
        assert_eq!(m.container, PathBuf::from("/bar"));
        assert_eq!(m.mode, MountMode::Ro);
    }

    #[test]
    fn three_part_ro() {
        let m = parse("/foo:/bar:ro").unwrap();
        assert_eq!(m.mode, MountMode::Ro);
    }

    #[test]
    fn three_part_rw_explicit() {
        let m = parse("/foo:/bar:rw").unwrap();
        assert_eq!(m.mode, MountMode::Rw);
    }

    #[test]
    fn accepts_paths_with_spaces() {
        let m = parse("/host with space:/cont with space:ro").unwrap();
        assert_eq!(m.host, PathBuf::from("/host with space"));
        assert_eq!(m.container, PathBuf::from("/cont with space"));
        assert_eq!(m.mode, MountMode::Ro);
    }

    #[test]
    fn accepts_multi_component_paths() {
        let m = parse("/a/b/c:/d/e/f").unwrap();
        assert_eq!(m.host, PathBuf::from("/a/b/c"));
        assert_eq!(m.container, PathBuf::from("/d/e/f"));
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse(""), Err(MountParseError::BadShape(String::new())));
    }

    #[test]
    fn rejects_no_colon() {
        assert_eq!(
            parse("/foo"),
            Err(MountParseError::BadShape("/foo".to_owned()))
        );
    }

    #[test]
    fn rejects_too_many_colons() {
        assert_eq!(
            parse("/foo:/bar:rw:extra"),
            Err(MountParseError::BadShape("/foo:/bar:rw:extra".to_owned()))
        );
    }

    #[test]
    fn rejects_bare_colon_as_empty_host() {
        assert_eq!(
            parse(":"),
            Err(MountParseError::EmptyPath {
                input: ":".to_owned(),
                field: "host",
            })
        );
    }

    #[test]
    fn rejects_empty_host() {
        assert_eq!(
            parse(":/bar"),
            Err(MountParseError::EmptyPath {
                input: ":/bar".to_owned(),
                field: "host",
            })
        );
    }

    #[test]
    fn rejects_empty_container() {
        assert_eq!(
            parse("/foo:"),
            Err(MountParseError::EmptyPath {
                input: "/foo:".to_owned(),
                field: "container",
            })
        );
    }

    #[test]
    fn rejects_empty_container_with_mode() {
        assert_eq!(
            parse("/foo::rw"),
            Err(MountParseError::EmptyPath {
                input: "/foo::rw".to_owned(),
                field: "container",
            })
        );
    }

    #[test]
    fn rejects_invalid_mode() {
        assert_eq!(
            parse("/foo:/bar:bogus"),
            Err(MountParseError::BadMode {
                input: "/foo:/bar:bogus".to_owned(),
                got: "bogus".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_uppercase_mode() {
        assert_eq!(
            parse("/foo:/bar:RW"),
            Err(MountParseError::BadMode {
                input: "/foo:/bar:RW".to_owned(),
                got: "RW".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_empty_mode() {
        assert_eq!(
            parse("/foo:/bar:"),
            Err(MountParseError::BadMode {
                input: "/foo:/bar:".to_owned(),
                got: String::new(),
            })
        );
    }

    #[test]
    fn rejects_relative_host() {
        assert_eq!(
            parse("foo:/bar"),
            Err(MountParseError::RelativePath {
                input: "foo:/bar".to_owned(),
                field: "host",
                path: "foo".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_dotted_relative_host() {
        assert!(matches!(
            parse("./foo:/bar"),
            Err(MountParseError::RelativePath { field: "host", .. })
        ));
    }

    #[test]
    fn rejects_relative_container() {
        assert_eq!(
            parse("/foo:bar"),
            Err(MountParseError::RelativePath {
                input: "/foo:bar".to_owned(),
                field: "container",
                path: "bar".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_both_relative() {
        // Host is checked first.
        assert!(matches!(
            parse("foo:bar"),
            Err(MountParseError::RelativePath { field: "host", .. })
        ));
    }

    #[test]
    fn volume_arg_renders_rw_with_z_flag() {
        let m = Mount {
            host: PathBuf::from("/foo"),
            container: PathBuf::from("/bar"),
            mode: MountMode::Rw,
        };
        assert_eq!(m.to_volume_arg().unwrap(), "/foo:/bar:rw,z");
    }

    #[test]
    fn volume_arg_renders_ro_with_z_flag() {
        let m = Mount {
            host: PathBuf::from("/foo"),
            container: PathBuf::from("/bar"),
            mode: MountMode::Ro,
        };
        assert_eq!(m.to_volume_arg().unwrap(), "/foo:/bar:ro,z");
    }

    #[test]
    fn parse_then_render_roundtrip_appends_z() {
        let m: Mount = "/host:/cont:ro".parse().unwrap();
        assert_eq!(m.to_volume_arg().unwrap(), "/host:/cont:ro,z");
    }

    #[test]
    fn volume_arg_rejects_colon_in_host() {
        let m = Mount {
            host: PathBuf::from("/has:colon"),
            container: PathBuf::from("/bar"),
            mode: MountMode::Ro,
        };
        assert_eq!(
            m.to_volume_arg(),
            Err(MountRenderError::PathContainsColon {
                field: "host",
                path: "/has:colon".to_owned(),
            })
        );
    }

    #[test]
    fn volume_arg_rejects_colon_in_container() {
        let m = Mount {
            host: PathBuf::from("/foo"),
            container: PathBuf::from("/has:colon"),
            mode: MountMode::Ro,
        };
        assert_eq!(
            m.to_volume_arg(),
            Err(MountRenderError::PathContainsColon {
                field: "container",
                path: "/has:colon".to_owned(),
            })
        );
    }

    #[test]
    fn mount_mode_as_str_matches_parser() {
        // Reverse of MountMode FromStr behaviour: as_str must be lowercase.
        assert_eq!(MountMode::Ro.as_str(), "ro");
        assert_eq!(MountMode::Rw.as_str(), "rw");
    }
}
