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

use phper::{values::{ExecuteData, ZVal}, objects::ZObj};
use crate::execute::get_this_mut;
use anyhow::Context;

/// Api style.
pub enum ApiStyle {
    /// Object-oriented.
    OO,
    /// Procedural.
    Procedural,
}

impl ApiStyle {
    pub fn get_this_mut(&self, execute_data: &mut ExecuteData) -> anyhow::Result<&mut ZObj> {
        match self {
            ApiStyle::OO => get_this_mut(execute_data),
            ApiStyle::Procedural => {
                execute_data.get_mut_parameter(0).as_mut_z_obj().context("first argument isn't object")
            }
        }
    }

    pub fn get_mut_parameter(&self, execute_data: &mut ExecuteData, index: usize) -> &mut ZVal {
        let index = match self {
            ApiStyle::OO => index,
            ApiStyle::Procedural => index + 1,
        };
        execute_data.get_mut_parameter(index)
    }
}
