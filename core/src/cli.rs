use clap::{Parser, Subcommand};
use std::{fmt::Display, path::PathBuf};

/// Modder is a tool for managing mods for Minecraft.
/// It can add mods from Modrinth and Github.
/// Other features include bulk-updating a directory of mods to a specified version
/// and listing information about the mods in a directory.
/// The `toggle` feature allows you to enable or disable
/// mods in a directory without having to remove them.
///
/// Modder is still in development and may have bugs.
/// Please report any issues on the GitHub repository.
///
///
/// Developed by JayanAXHF
#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    /// Whether to print the output to the console. If `false`, only error messages will be printed
    #[arg(short, long, default_value_t = false)]
    pub silent: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Add a mod to the supplied directory (defaults to current directory)
    #[command(arg_required_else_help = true)]
    Add {
        /// The mod name
        #[arg(required = true)]
        mod_: String,
        /// The game version to add this mod for
        #[arg(short, long)]
        version: Option<String>,
        /// Where to download the mod from
        #[arg(short, long)]
        source: Option<Source>,
        /// Github token for any mods nested in a github repo.
        #[arg(short, long)]
        token: Option<String>,
    },
    /// Bulk-update a directory of mods to the specified version
    #[command(arg_required_else_help = true)]
    Update {
        /// The directory to update mods in
        #[arg( default_value_os_t = PathBuf::from("./"))]
        dir: PathBuf,
        /// The game version to add this mod for
        #[arg(short, long)]
        version: Option<String>,
        #[arg(short, long)]
        delete_previous: bool,
        /// Github token for any mods nested in a github repo.
        #[arg(short, long)]
        token: Option<String>,
    },
    /// Quickly add mods from a curated list to the supplied directory (defaults to current directory)
    QuickAdd {
        /// The game version to add this mod for
        #[arg(short, long)]
        version: Option<String>,
        /// Find top `limit` mods from Modrinth
        #[arg(short, long, default_value_t = 100)]
        limit: u16,
    },
    /// All the other options, just run in the minecraft directory
    InPlace {
        /// The game version to add this mod for
        #[arg(short, long)]
        version: Option<String>,
        /// Passed down to the quick add command
        #[arg(short, long, default_value_t = 100)]
        limit: u16,
    },
    /// Toggle a mod in the supplied directory (defaults to current directory)
    Toggle {
        /// The game version to add this mod for
        #[arg(short, long)]
        version: Option<String>,
        /// The directory to toggle mods in
        #[arg(short, long, default_value_os_t = PathBuf::from("./"))]
        dir: PathBuf,
    },
    /// List all the mods in the supplied directory (defaults to current directory)
    List {
        /// The directory to list mods in
        #[arg(default_value_os_t = PathBuf::from("./"))]
        dir: PathBuf,
        /// Whether to print verbose imformation
        #[arg(short, long, default_value_t = false)]
        verbose: bool,
    },
}

impl Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Commands::QuickAdd { .. } => "Quick Add".to_string(),
            Commands::Update { .. } => "Update".to_string(),
            Commands::Add { .. } => "Add".to_string(),
            Commands::InPlace { .. } => "Edit Minecraft Directory".to_string(),
            Commands::Toggle { .. } => "Toggle".to_string(),
            Commands::List { .. } => "List".to_string(),
        };
        write!(f, "{}", text)
    }
}

#[derive(Debug, Clone, clap::ValueEnum, PartialEq, Default, Hash, Eq)]
pub enum Source {
    #[default]
    Modrinth,
    Github,
}

impl Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Source::Modrinth => "modrinth".to_string(),
            Source::Github => "github".to_string(),
        };
        write!(f, "{}", text)
    }
}

impl TryInto<Source> for &str {
    type Error = String;
    fn try_into(self) -> Result<Source, Self::Error> {
        match self.trim().to_lowercase().as_str() {
            "modrinth" => Ok(Source::Modrinth),
            "github" => Ok(Source::Github),
            _ => Err("Invalid source".to_string()),
        }
    }
}
