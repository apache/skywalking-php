mod create_package_xml;

use self::create_package_xml::CreatePackageXmlCommand;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum Commands {
    CreatePackageXml(CreatePackageXmlCommand),
}

impl Commands {
    pub fn run(&self) -> anyhow::Result<()> {
        match self {
            Commands::CreatePackageXml(cmd) => cmd.run(),
        }
    }
}
