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
    send::{run as run_send, Arguments as SendArguments},
    shell::{run as run_shell, Arguments as ShellArguments},
    start::{run as run_start, Arguments as StartArguments},
    stop::{run as run_stop, Arguments as StopArguments},
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
