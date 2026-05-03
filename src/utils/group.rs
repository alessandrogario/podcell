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
pub enum EtcGroupError {
    #[error("failed to parse the group file")]
    IOError(#[from] io::Error),

    #[error("invalid group file entry")]
    InvalidEntryFormat,
}

pub struct Group {
    pub name: String,
    pub id: u32,
}

pub struct EtcGroup {
    group_list: Vec<Group>,
}

impl EtcGroup {
    pub fn new<AsPathRef: AsRef<Path>>(path: AsPathRef) -> Result<Self, EtcGroupError> {
        Ok(Self {
            group_list: Self::parse_file(path)?,
        })
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Group> {
        self.group_list.iter()
    }

    fn parse_file<AsPathRef: AsRef<Path>>(path: AsPathRef) -> Result<Vec<Group>, EtcGroupError> {
        let mut group_list = Vec::new();

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;

            let field_list: Vec<&str> = line.split(':').collect();
            if field_list.len() != 4 {
                return Err(EtcGroupError::InvalidEntryFormat);
            }

            let group = Group {
                name: field_list[0].to_string(),
                id: field_list[2].parse().unwrap_or(0),
            };

            group_list.push(group);
        }

        Ok(group_list)
    }
}
