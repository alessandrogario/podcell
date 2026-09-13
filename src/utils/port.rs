//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use thiserror::Error;

use std::str::FromStr;

/// Transport protocol for a published port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortProtocol {
    /// Transmission Control Protocol.
    Tcp,
    /// User Datagram Protocol.
    Udp,
}

impl PortProtocol {
    /// Returns the protocol name accepted by Podman.
    pub fn as_str(self) -> &'static str {
        match self {
            PortProtocol::Tcp => "tcp",
            PortProtocol::Udp => "udp",
        }
    }
}

/// Errors returned when parsing a port binding.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PortParseError {
    /// The binding does not match `HOST:CONTAINER[/PROTOCOL]`.
    #[error("invalid port '{0}': expected HOST:CONTAINER[/PROTOCOL]")]
    BadShape(String),

    /// The host or container port is missing.
    #[error("invalid port '{input}': missing {field} port")]
    MissingPort { input: String, field: &'static str },

    /// The host or container port is not a valid `u16`.
    #[error("invalid port '{input}': {field} port '{value}' is not a valid port number")]
    InvalidPort {
        /// Original binding string.
        input: String,
        /// Name of the invalid field.
        field: &'static str,
        /// Invalid field value.
        value: String,
    },

    /// The host or container port is zero.
    #[error("invalid port '{input}': {field} port must not be 0 (random ports are not supported)")]
    ZeroPort { input: String, field: &'static str },

    /// The protocol is not supported.
    #[error("invalid port '{input}': protocol must be 'tcp' or 'udp', got '{got}'")]
    BadProtocol { input: String, got: String },
}

/// Format: `HOST:CONTAINER[/PROTOCOL]`, e.g. `8080:80/tcp` or `8080:80` (defaults to
/// TCP). Host and container values are port numbers in the 1-65535 range; port 0
/// (ephemeral/random) is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port {
    /// Port exposed on the host.
    pub host: u16,
    /// Port receiving traffic in the container.
    pub container: u16,
    /// Transport protocol for the binding.
    pub protocol: PortProtocol,
}

impl Port {
    /// Renders the binding as a Podman `--publish` value.
    pub fn to_podman_publish_arg(self) -> String {
        format!(
            "{}:{}/{}",
            self.host,
            self.container,
            self.protocol.as_str()
        )
    }
}

impl FromStr for Port {
    type Err = PortParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let host_str = match parts.next() {
            Some(part) => part,
            None => return Err(PortParseError::BadShape(s.to_owned())),
        };

        let rest = match parts.next() {
            Some(part) => part,
            None => return Err(PortParseError::BadShape(s.to_owned())),
        };

        if parts.next().is_some() {
            return Err(PortParseError::BadShape(s.to_owned()));
        }

        let (container_str, protocol) = match rest.split_once('/') {
            Some((container, protocol)) => (container, parse_protocol(protocol, s)?),
            None => (rest, PortProtocol::Tcp),
        };

        if host_str.is_empty() {
            return Err(PortParseError::MissingPort {
                input: s.to_owned(),
                field: "host",
            });
        }

        if container_str.is_empty() {
            return Err(PortParseError::MissingPort {
                input: s.to_owned(),
                field: "container",
            });
        }

        let host = parse_numeric_port(host_str, s, "host")?;
        let container = parse_numeric_port(container_str, s, "container")?;

        Ok(Port {
            host,
            container,
            protocol,
        })
    }
}

/// Parses a nonzero port number.
fn parse_numeric_port(s: &str, input: &str, field: &'static str) -> Result<u16, PortParseError> {
    let port = s.parse::<u16>().map_err(|_| PortParseError::InvalidPort {
        input: input.to_owned(),
        field,
        value: s.to_owned(),
    })?;

    if port == 0 {
        return Err(PortParseError::ZeroPort {
            input: input.to_owned(),
            field,
        });
    }

    Ok(port)
}

/// Parses a supported protocol name.
fn parse_protocol(s: &str, input: &str) -> Result<PortProtocol, PortParseError> {
    match s {
        "tcp" => Ok(PortProtocol::Tcp),
        "udp" => Ok(PortProtocol::Udp),
        _ => Err(PortParseError::BadProtocol {
            input: input.to_owned(),
            got: s.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Port, PortParseError> {
        s.parse()
    }

    #[test]
    fn defaults_to_tcp_without_protocol() {
        let p = parse("8080:80").unwrap();
        assert_eq!(p.host, 8080);
        assert_eq!(p.container, 80);
        assert_eq!(p.protocol, PortProtocol::Tcp);
    }

    #[test]
    fn parses_tcp_protocol() {
        let p = parse("8080:80/tcp").unwrap();
        assert_eq!(p.protocol, PortProtocol::Tcp);
    }

    #[test]
    fn parses_udp_protocol() {
        let p = parse("8080:80/udp").unwrap();
        assert_eq!(p.protocol, PortProtocol::Udp);
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse(""), Err(PortParseError::BadShape(String::new())));
    }

    #[test]
    fn rejects_no_colon() {
        assert_eq!(
            parse("8080"),
            Err(PortParseError::BadShape("8080".to_owned()))
        );
    }

    #[test]
    fn rejects_too_many_colons() {
        assert_eq!(
            parse("8080:80:extra"),
            Err(PortParseError::BadShape("8080:80:extra".to_owned()))
        );
    }

    #[test]
    fn rejects_missing_host_port() {
        assert_eq!(
            parse(":80"),
            Err(PortParseError::MissingPort {
                input: ":80".to_owned(),
                field: "host",
            })
        );
    }

    #[test]
    fn rejects_missing_container_port() {
        assert_eq!(
            parse("8080:"),
            Err(PortParseError::MissingPort {
                input: "8080:".to_owned(),
                field: "container",
            })
        );
    }

    #[test]
    fn rejects_missing_container_with_protocol() {
        assert_eq!(
            parse("8080:/tcp"),
            Err(PortParseError::MissingPort {
                input: "8080:/tcp".to_owned(),
                field: "container",
            })
        );
    }

    #[test]
    fn rejects_non_numeric_host() {
        assert_eq!(
            parse("abc:80"),
            Err(PortParseError::InvalidPort {
                input: "abc:80".to_owned(),
                field: "host",
                value: "abc".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_out_of_range_port() {
        assert_eq!(
            parse("70000:80"),
            Err(PortParseError::InvalidPort {
                input: "70000:80".to_owned(),
                field: "host",
                value: "70000".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_zero_host_port() {
        assert_eq!(
            parse("0:80"),
            Err(PortParseError::ZeroPort {
                input: "0:80".to_owned(),
                field: "host",
            })
        );
    }

    #[test]
    fn rejects_zero_container_port() {
        assert_eq!(
            parse("8080:0"),
            Err(PortParseError::ZeroPort {
                input: "8080:0".to_owned(),
                field: "container",
            })
        );
    }

    #[test]
    fn rejects_invalid_protocol() {
        assert_eq!(
            parse("8080:80/sctp"),
            Err(PortParseError::BadProtocol {
                input: "8080:80/sctp".to_owned(),
                got: "sctp".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_uppercase_protocol() {
        assert_eq!(
            parse("8080:80/TCP"),
            Err(PortParseError::BadProtocol {
                input: "8080:80/TCP".to_owned(),
                got: "TCP".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_empty_protocol() {
        assert_eq!(
            parse("8080:80/"),
            Err(PortParseError::BadProtocol {
                input: "8080:80/".to_owned(),
                got: String::new(),
            })
        );
    }

    #[test]
    fn renders_publish_arg() {
        let p = parse("8080:80/tcp").unwrap();
        assert_eq!(p.to_podman_publish_arg(), "8080:80/tcp");
    }

    #[test]
    fn parse_then_render_roundtrip_udp() {
        let p: Port = "53:53/udp".parse().unwrap();
        assert_eq!(p.to_podman_publish_arg(), "53:53/udp");
    }

    #[test]
    fn port_protocol_as_str_matches_parser() {
        assert_eq!(PortProtocol::Tcp.as_str(), "tcp");
        assert_eq!(PortProtocol::Udp.as_str(), "udp");
    }
}
