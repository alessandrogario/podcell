//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use std::{
    io,
    path::{Path, PathBuf},
};

pub fn which<P: AsRef<Path>>(command: P) -> io::Result<Option<PathBuf>> {
    let path_var = std::env::var("PATH").map_err(|error| {
        io::Error::other(format!(
            "Failed to access the PATH environment variable: {error:?}"
        ))
    })?;

    for dir in path_var.split(':') {
        let bin_path = Path::new(dir).join(&command);
        if bin_path.exists() && bin_path.is_file() {
            return Ok(Some(bin_path));
        }
    }

    Ok(None)
}
