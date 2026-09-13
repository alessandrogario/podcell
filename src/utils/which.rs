//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use thiserror::Error;

use std::path::{Path, PathBuf};

/// Errors produced while searching for executables in `PATH`.
#[derive(Debug, Error)]
pub enum WhichError {
    #[error("failed to access the PATH environment variable")]
    PathVariable(#[source] std::env::VarError),
}

pub fn which<P: AsRef<Path>>(command: P) -> Result<Option<PathBuf>, WhichError> {
    let path_var = std::env::var("PATH").map_err(WhichError::PathVariable)?;

    for dir in path_var.split(':') {
        let bin_path = Path::new(dir).join(&command);
        if bin_path.exists() && bin_path.is_file() {
            return Ok(Some(bin_path));
        }
    }

    Ok(None)
}
