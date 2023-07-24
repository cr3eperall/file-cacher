use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(arg_required_else_help(true),author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: SubCommand,
}

#[derive(Subcommand, Debug)]
pub enum SubCommand {
    #[command(arg_required_else_help(true))]
    Get {
        #[arg(short = 'o', long = "output")]
        filename: String,

        url: String,
        #[arg(short, long)]
        refresh: bool,
    },
    Stats,
    #[command(about = "remove expired files")]
    CleanExpired,
    #[command(about = "delete all the files in the cache")]
    Delete,
}
