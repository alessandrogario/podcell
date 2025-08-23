//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

mod commands;
mod utils;

use crate::commands::{
    create::{run as run_create, Arguments as CreateArguments},
    enter::{run as run_enter, Arguments as EnterArguments},
    init::{run as run_init, Arguments as InitArguments},
    list::{run as run_list, Arguments as ListArguments},
    rm::{run as run_rm, Arguments as RmArguments},
};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "devshell")]
#[command(about = "A simple podman-based development environment manager.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Create(CreateArguments),
    Enter(EnterArguments),
    List(ListArguments),
    Rm(RmArguments),
    #[clap(hide = true)]
    Init(InitArguments),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Create(args) => run_create(args),
        Command::Enter(args) => run_enter(args),
        Command::List(args) => run_list(args),
        Command::Init(args) => run_init(args),
        Command::Rm(args) => run_rm(args),
    }
}
