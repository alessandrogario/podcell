//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::Podman;

use clap::{arg, Args};

/// Delete an existing container.
#[derive(Args)]
pub struct Arguments {
    /// The name of the container to delete.
    #[arg()]
    name: String,
}

/// Handler for the "rm" command.
pub fn run(args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    Podman::new().rm(&args.name).map_err(Into::into)
}
