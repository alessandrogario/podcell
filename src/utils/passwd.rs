//
// Copyright (c) 2025-present, Alessandro Gario
// All rights reserved.
//
// This source code is licensed in accordance with the terms specified in
// the LICENSE file found in the root directory of this source tree.
//

use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EtcPasswdError {
    #[error("failed to parse the passwd file")]
    IOError(#[from] io::Error),

    #[error("invalid passwd file entry")]
    InvalidEntryFormat,
}

pub struct User {
    pub name: String,
    pub id: u32,
    pub group_id: u32,
    pub user_information: String,
    pub home_path: String,
    pub shell: String,
}

pub struct EtcPasswd {
    user_list: Vec<User>,
}

impl EtcPasswd {
    pub fn new<AsPathRef: AsRef<Path>>(path: AsPathRef) -> Result<Self, EtcPasswdError> {
        Ok(Self {
            user_list: Self::parse_file(path)?,
        })
    }

    pub fn len(&self) -> usize {
        self.user_list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.user_list.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, User> {
        self.user_list.iter()
    }

    fn parse_file<AsPathRef: AsRef<Path>>(path: AsPathRef) -> Result<Vec<User>, EtcPasswdError> {
        let mut user_list = Vec::new();

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;

            let field_list: Vec<&str> = line.split(':').collect();
            if field_list.len() != 7 {
                return Err(EtcPasswdError::InvalidEntryFormat);
            }

            let user = User {
                name: field_list[0].to_string(),
                id: field_list[2].parse().unwrap_or(0),
                group_id: field_list[3].parse().unwrap_or(0),
                user_information: field_list[4].to_string(),
                home_path: field_list[5].to_string(),
                shell: field_list[6].to_string(),
            };

            user_list.push(user);
        }

        Ok(user_list)
    }
}
