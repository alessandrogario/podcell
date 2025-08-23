//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use crate::utils::podman::Podman;

use clap::Args;

/// List all available containers.
#[derive(Args)]
pub struct Arguments {}

/// Handler for the "list" command.
pub fn run(_args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    let podman = Podman::new();

    let container_list = podman.list()?;
    if container_list.is_empty() {
        println!("No devshell-managed containers found.");
        return Ok(());
    }

    println!(
        "{:<12} {:<30} {:<10} {:<12} {:<164}",
        "CONTAINER ID", "NAME", "STATE", "IMAGE ID", "IMAGE"
    );

    println!(
        "{:-<12} {:-<30} {:-<10} {:-<12} {:-<64}",
        "", "", "", "", ""
    );

    for container in &container_list {
        let state = format!("{:?}", container.state);
        let name = container
            .name_list
            .first()
            .map(String::as_str)
            .unwrap_or("<unnamed>");

        let container_id = if container.id.len() > 12 {
            &container.id[..12]
        } else {
            &container.id
        };

        let image_id = if container.image_id.len() > 12 {
            &container.image_id[..12]
        } else {
            &container.image_id
        };

        println!(
            "{:<12} {:<30} {:<10} {:<12} {:<64}",
            container_id, name, state, image_id, container.image
        );
    }

    Ok(())
}
