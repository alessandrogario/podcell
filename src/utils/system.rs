//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

//! Host-side user and path helpers.

use crate::utils::passwd::{EtcPasswd, EtcPasswdError};

use thiserror::Error;

use std::{io, os::unix::fs::MetadataExt, path::Path};

/// System passwd file used for user lookup.
const ETC_PASSWD_PATH: &str = "/etc/passwd";

/// Errors produced while looking up host users and filesystem ownership.
#[derive(Debug, Error)]
pub enum SystemError {
    /// A required environment variable could not be read.
    #[error("failed to access the {name} environment variable: {source}")]
    EnvironmentVariable {
        name: &'static str,
        #[source]
        source: std::env::VarError,
    },

    /// The host passwd database could not be read or parsed.
    #[error("failed to read the host passwd database: {0}")]
    Passwd(#[from] EtcPasswdError),

    /// The requested user does not exist in the host passwd database.
    #[error("user '{username}' not found in {ETC_PASSWD_PATH}")]
    UserNotFound { username: String },

    /// Metadata for a host path could not be read.
    #[error("mount host path '{path}' is not accessible: {source}")]
    PathMetadata {
        path: String,
        #[source]
        source: io::Error,
    },
}

/// Looks up the current user's UID by reading `$USER` and resolving it via `/etc/passwd`.
pub fn current_user_uid() -> Result<u32, SystemError> {
    let username = std::env::var("USER").map_err(|source| SystemError::EnvironmentVariable {
        name: "USER",
        source,
    })?;

    let etc_passwd = EtcPasswd::new(ETC_PASSWD_PATH)?;

    etc_passwd
        .iter()
        .find(|user| user.name == username)
        .map(|user| user.id)
        .ok_or(SystemError::UserNotFound { username })
}

/// Checks whether a path is owned by the expected user ID.
pub fn is_path_owned_by_user(path: &Path, expected_uid: u32) -> Result<bool, SystemError> {
    let metadata = std::fs::metadata(path).map_err(|source| SystemError::PathMetadata {
        path: path.display().to_string(),
        source,
    })?;

    let owner_uid = metadata.uid();
    Ok(owner_uid == expected_uid)
}
