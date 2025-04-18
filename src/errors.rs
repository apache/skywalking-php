// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::anyhow;
use std::{result, str::Utf8Error};

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    PHPer(#[from] phper::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Self {
        Self::Anyhow(e.into())
    }
}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Self::Anyhow(e.into())
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Self::Anyhow(anyhow!("{}", e))
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Self::Anyhow(anyhow!("{}", e))
    }
}
