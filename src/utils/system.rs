//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

//! Host-side user and path helpers.

use std::{io, os::unix::fs::MetadataExt, path::Path};

use crate::utils::passwd::EtcPasswd;

/// System passwd file used for user lookup.
const ETC_PASSWD_PATH: &str = "/etc/passwd";

/// Looks up the current user's UID by reading `$USER` and resolving it via `/etc/passwd`.
pub fn current_user_uid() -> io::Result<u32> {
    let username = std::env::var("USER")
        .map_err(|err| io::Error::other(format!("USER environment variable not set: {err}")))?;

    let etc_passwd =
        EtcPasswd::new(ETC_PASSWD_PATH).map_err(|err| io::Error::other(format!("{err}")))?;

    etc_passwd
        .iter()
        .find(|user| user.name == username)
        .map(|user| user.id)
        .ok_or_else(|| {
            io::Error::other(format!("User '{username}' not found in {ETC_PASSWD_PATH}"))
        })
}

/// Checks whether a path is owned by the expected user ID.
pub fn is_path_owned_by_user(path: &Path, expected_uid: u32) -> io::Result<bool> {
    let metadata = std::fs::metadata(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "Mount host path '{}' is not accessible: {err}",
                path.display()
            ),
        )
    })?;

    let owner_uid = metadata.uid();
    Ok(owner_uid == expected_uid)
}
