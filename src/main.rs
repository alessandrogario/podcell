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
    create::{Arguments as CreateArguments, run as run_create},
    enter::{Arguments as EnterArguments, run as run_enter},
    init::{Arguments as InitArguments, run as run_init},
    list::{Arguments as ListArguments, run as run_list},
    rm::{Arguments as RmArguments, run as run_rm},
    send::{Arguments as SendArguments, run as run_send},
    shell::{Arguments as ShellArguments, run as run_shell},
    start::{Arguments as StartArguments, run as run_start},
    stop::{Arguments as StopArguments, run as run_stop},
};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "podcell")]
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
    Send(SendArguments),
    Start(StartArguments),
    Stop(StopArguments),
    #[clap(hide = true)]
    Init(InitArguments),
    #[clap(hide = true)]
    Shell(ShellArguments),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Create(args) => run_create(args),
        Command::Enter(args) => run_enter(args),
        Command::List(args) => run_list(args),
        Command::Init(args) => run_init(args),
        Command::Rm(args) => run_rm(args),
        Command::Send(args) => run_send(args),
        Command::Shell(args) => run_shell(args),
        Command::Start(args) => run_start(args),
        Command::Stop(args) => run_stop(args),
    }
}
