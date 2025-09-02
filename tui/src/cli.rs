use std::path::PathBuf;

use clap::Parser;

use crate::config::get_data_dir;

#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
pub struct Cli {
    #[arg(short, long, default_value_os_t = PathBuf::from("./"))]
    pub dir: PathBuf,
}

const VERSION_MESSAGE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("VERGEN_GIT_DESCRIBE"),
    " (",
    env!("VERGEN_BUILD_DATE"),
    ")"
);

pub fn version() -> String {
    let author = clap::crate_authors!();

    // let current_exe_path = PathBuf::from(clap::crate_name!()).display().to_string();
    let data_dir_path = get_data_dir().display().to_string();

    format!(
        "\
{VERSION_MESSAGE}

Authors: {author}

Data directory: {data_dir_path}"
    )
}
