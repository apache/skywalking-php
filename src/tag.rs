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
// limitations under the License..

//! Tags
//!
//! Virtual Cache
//!
//! <https://skywalking.apache.org/docs/main/next/en/setup/service-agent/virtual-cache/>
//!
//! Virtual Database
//!
//! <https://skywalking.apache.org/docs/main/next/en/setup/service-agent/virtual-database/>

use std::fmt::Display;

pub const TAG_CACHE_TYPE: &str = "cache.type";
pub const TAG_CACHE_OP: &str = "cache.op";
pub const TAG_CACHE_CMD: &str = "cache.cmd";
pub const TAG_CACHE_KEY: &str = "cache.key";

pub enum CacheOp {
    Read,
    Write,
}

impl Display for CacheOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
        }
    }
}

pub const TAG_DB_STATEMENT: &str = "db.statement";
pub const TAG_DB_TYPE: &str = "db.type";

pub const TAG_MQ_BROKER: &str = "mq.broker";
pub const TAG_MQ_TOPIC: &str = "mq.topic";
pub const TAG_MQ_QUEUE: &str = "mq.queue";
