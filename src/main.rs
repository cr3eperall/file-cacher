use std::io::{self, Write};

use clap::Parser;
use file_cacher::{
    cli::{self, Cli},
    Cacher,
};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    //println!("{:#?}", cli);

    let mut cacher = Cacher::new(None);

    match cli.command {
        cli::SubCommand::Get {
            filename,
            url,
            refresh,
        } => {
            let res = cacher.get(url, &filename, refresh).await;
            match res {
                Ok(path) => writeln!(io::stdout(), "{}", path).expect("error writing to stdout"),
                Err(err) => {
                    writeln!(io::stderr(), "Error:{:#?}", err).expect("error writing to stderr")
                }
            }
            if let Err(err) = cacher.save() {
                writeln!(io::stderr(), "error saving cache file:{:#?}", err)
                    .expect("error writing to stderr");
            }
        }
        cli::SubCommand::Stats => {
            let stats = cacher.stats();
            if stats.number_of_cached_files == 0 {
                writeln!(io::stdout(), "cache is empty").expect("error writing to stdout");
            } else {
                writeln!(io::stdout(), "{}", stats).expect("error writing to stdout");
            }
        }
        cli::SubCommand::CleanExpired => {
            let count = cacher.clean_expired();
            writeln!(io::stdout(), "{} files removed", count).expect("error writing to stdout");
        }
        cli::SubCommand::Delete => {
            match cacher.clear() {
                Ok(count) => writeln!(io::stdout(), "{} files removed", count).expect("error writing to stdout"),
                Err(err) => {
                    writeln!(io::stderr(), "error deleting cache:{:#?}", err)
                    .expect("error writing to stderr");
                },
            }
        }
    }
}
