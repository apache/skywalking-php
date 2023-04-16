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

use chrono::{DateTime, Local};
use clap::Parser;
use serde::Serialize;
use std::{fs, path::PathBuf, process::Command, time::SystemTime};
use tera::{Context, Tera};
use tracing::info;

/// Create package.xml from template file.
#[derive(Parser, Debug)]
pub struct CreatePackageXmlCommand {
    /// Template file path.
    #[clap(long, default_value = "./package.tpl.xml")]
    tpl_path: PathBuf,

    /// Target file path.
    #[clap(long, default_value = "./package.xml")]
    target_path: PathBuf,

    /// Project directory path.
    #[clap(long, default_value = ".")]
    project_path: PathBuf,

    /// Version of skywalking_agent.
    #[clap(long)]
    version: String,

    /// Release date, default is current local timezone date.
    #[clap(long)]
    date: Option<String>,

    /// Release notes.
    #[clap(long)]
    notes: String,
}

#[derive(Serialize)]
struct File {
    name: String,
    role: String,
}

impl File {
    fn new(path: &str) -> Self {
        let path = path.trim_matches('"');
        let role = if path.ends_with(".md")
            || path.starts_with("docs/")
            || path.starts_with("dist-material/")
            || ["LICENSE", "NOTICE"].contains(&path)
        {
            "doc"
        } else {
            "src"
        };

        Self {
            name: path.to_owned(),
            role: role.to_owned(),
        }
    }
}

impl CreatePackageXmlCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        info!(tpl_path = ?&self.tpl_path, "read template content");
        let tpl = fs::read_to_string(&self.tpl_path)?;

        let mut context = Context::new();
        context.insert("date", &self.get_date());
        context.insert("version", &self.version);
        context.insert("notes", &self.notes);
        context.insert("files", &self.get_git_files()?);

        let mut tera = Tera::default();
        let contents = tera.render_str(&tpl, &context)?;

        info!(target_path = ?&self.target_path, "write target content");
        fs::write(&self.target_path, contents)?;

        Ok(())
    }

    fn get_date(&self) -> String {
        match &self.date {
            Some(date) => date.to_owned(),
            None => {
                let datetime: DateTime<Local> = SystemTime::now().into();
                datetime.format("%Y-%m-%d").to_string()
            }
        }
    }

    fn get_git_files(&self) -> anyhow::Result<Vec<File>> {
        let output = Command::new("git")
            .args(["ls-tree", "-r", "HEAD", "--name-only"])
            .output()?;
        let content = String::from_utf8(output.stdout)?;
        Ok(content.split_whitespace().map(File::new).collect())
    }
}
