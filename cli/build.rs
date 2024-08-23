include!("src/cli.rs");

use clap::{CommandFactory, ValueEnum};
use clap_complete::Shell;
use clap_mangen::Man;
use std::env::var_os;
use std::fs::create_dir_all;
use std::io::Result;

fn main() -> Result<()> {
    let out_dir = PathBuf::from(var_os("OUT_DIR").unwrap());
    let mut app = Args::command();

    let complete_dir = out_dir.join("complete");
    create_dir_all(&complete_dir)?;
    for shell in Shell::value_variants() {
        clap_complete::generate_to(*shell, &mut app, "oxigraph", &complete_dir)?;
    }

    let man_dir = out_dir.join("man");
    create_dir_all(&man_dir)?;
    Man::new(app.clone().disable_help_subcommand(true)).generate_to(&man_dir)?;
    for subcommand in app.get_subcommands() {
        Man::new(subcommand.clone().disable_help_subcommand(true)).generate_to(&man_dir)?;
    }

    Ok(())
}
