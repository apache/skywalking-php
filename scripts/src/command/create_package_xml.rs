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
    path: String,
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

        let contents = Tera::one_off(&tpl, &context, false)?;
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
        Ok(content
            .split_whitespace()
            .map(|path| File {
                path: path.to_owned(),
            })
            .collect())
    }
}
