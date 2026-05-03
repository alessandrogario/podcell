//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

//! Host-side helpers for identifying the current user and validating
//! mount source paths before they are handed to podman.

use std::{io, os::unix::fs::MetadataExt, path::Path};

use crate::utils::passwd::EtcPasswd;

const ETC_PASSWD_PATH: &str = "/etc/passwd";

/// Looks up the current user's UID by reading `$USER` and resolving it via `/etc/passwd`.
///
/// We use `/etc/passwd` rather than a `getuid(2)` syscall because everything else in this
/// crate already goes through `EtcPasswd` for user/group resolution; keeping a single
/// source of truth avoids a divergence where the syscall and the file disagree (e.g. on
/// systems with NSS-backed users that aren't in `/etc/passwd`, both paths fail uniformly
/// instead of one silently succeeding).
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

/// Best-effort validation that a host bind-mount source path is safe to expose
/// into the container
///
/// A path is acceptable when it exists and is owned by the calling user. The ownership
/// check is the practical guard against `,z` SELinux relabeling on system-owned paths:
/// in rootless podman the relabel only succeeds for files we can already `chcon`, so
/// restricting to user-owned paths confines relabel side effects to user data.
pub fn validate_host_path(path: &Path, expected_uid: u32) -> io::Result<()> {
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
    if owner_uid != expected_uid {
        return Err(io::Error::other(format!(
            "Mount host path '{}' is owned by uid {owner_uid}, not the current user (uid {expected_uid}). \
             Refusing to mount: SELinux relabeling (:z) and rootless userns mapping both assume the path is user-owned.",
            path.display()
        )));
    }

    Ok(())
}
