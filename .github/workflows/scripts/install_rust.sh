#!/usr/bin/env bash

#
# Copyright (c) 2024-present, Alessandro Gario
# All rights reserved.
#
# This source code is licensed in accordance with the terms specified in
# the LICENSE file found in the root directory of this source tree.
#

main() {
  if ! command -v "curl" ; then
    printf "The 'curl' command is not available. Please install it and try again.\n"
    return 1
  fi

  if ! curl --proto '=https' \
            --tlsv1.2 \
            -sSf \
            https://sh.rustup.rs > "rustup.sh" ; then

    printf "Failed to download the rustup script\n"
    return 1
  fi

  if ! chmod +x "rustup.sh" ; then
    printf "Failed to set the rustup setup script as executable\n"
    return 1
  fi

  if ! ./rustup.sh -y --quiet ; then
    printf "Failed to install Rust.\n"
    return 1
  fi

  if command -v "apt-get" ; then
    sudo apt-get install musl-tools
  else
    sudo dnf install musl-devel
  fi

  if ! rustup target add x86_64-unknown-linux-musl ; then
    printf "Failed to install the musl target\n"
    return 1
  fi

  return 0
}

main $@
exit $?
