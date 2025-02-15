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

use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PsrLogLevel {
    Off,
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

impl From<&str> for PsrLogLevel {
    fn from(s: &str) -> Self {
        match &*s.to_uppercase() {
            "DEBUG" => PsrLogLevel::Debug,
            "INFO" => PsrLogLevel::Info,
            "NOTICE" => PsrLogLevel::Notice,
            "WARNING" => PsrLogLevel::Warning,
            "ERROR" => PsrLogLevel::Error,
            "CRITICAL" => PsrLogLevel::Critical,
            "ALERT" => PsrLogLevel::Alert,
            "EMERGENCY" => PsrLogLevel::Emergency,
            _ => PsrLogLevel::Off,
        }
    }
}

impl Display for PsrLogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PsrLogLevel::Off => "OFF".fmt(f),
            PsrLogLevel::Debug => "DEBUG".fmt(f),
            PsrLogLevel::Info => "INFO".fmt(f),
            PsrLogLevel::Notice => "NOTICE".fmt(f),
            PsrLogLevel::Warning => "WARNING".fmt(f),
            PsrLogLevel::Error => "ERROR".fmt(f),
            PsrLogLevel::Critical => "CRITICAL".fmt(f),
            PsrLogLevel::Alert => "ALERT".fmt(f),
            PsrLogLevel::Emergency => "EMERGENCY".fmt(f),
        }
    }
}
