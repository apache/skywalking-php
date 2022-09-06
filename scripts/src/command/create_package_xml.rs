use clap::Parser;
use serde::Serialize;
use std::{fs, path::PathBuf};
use tera::{Context, Tera};

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
        let tpl = fs::read_to_string(&self.tpl_path)?;

        let mut context = Context::new();
        context.insert("date", "0000-00-00");
        context.insert("version", &self.version);
        context.insert("notes", &self.notes);
        context.insert("files", &self.git_files());

        let contents = Tera::one_off(&tpl, &context, true)?;
        fs::write(&self.target_path, contents)?;

        Ok(())
    }

    fn git_files(&self) -> Vec<File> {
        vec![]
    }
}
