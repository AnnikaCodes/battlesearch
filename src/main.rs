/// Battlesearch code for Pokémon Showdown battle logs
mod search;

use search::{BattleSearchError, BattleSearcher, ToSend};
use std::{path::PathBuf, sync::mpsc, thread};
use structopt::StructOpt;

const PIKKR_TRAINING_ROUNDS: usize = 2;

fn get_filename(file: &PathBuf) -> Result<String, BattleSearchError> {
    match file.file_name() {
        Some(os_str) => match os_str.to_str() {
            Some(s) => Ok(String::from(s)),
            None => {
                return Err(BattleSearchError::Path(format!(
                    "Couldn't get filename of {:?}",
                    file
                )))
            }
        },
        None => {
            return Err(BattleSearchError::Path(format!(
                "Couldn't get filename of {:?}",
                file
            )))
        }
    }
}

fn handle_dir(
    directory: &PathBuf,
    threads: &Vec<mpsc::Sender<ToSend>>,
) -> Result<(), BattleSearchError> {
    let mut current_sender_idx = 0;
    let num_threads = threads.len();

    let contents = directory.read_dir()?;
    let date = get_filename(directory)?;
    for entry in contents {
        if let Ok(file) = entry {
            if file.file_type()?.is_dir() {
                handle_dir(&file.path(), &threads)?;
            } else {
                threads
                    .get(current_sender_idx)
                    .unwrap()
                    .send(ToSend::File(file.path(), date.clone()))
                    .unwrap_or_else(|e| {
                        println!("{:?}", e);
                    });
                current_sender_idx = (current_sender_idx + 1) % num_threads;
            }
        }
    }

    Ok(())
}

#[derive(StructOpt)]
#[structopt(
    rename_all = "kebab-case",
    author = "Annika L.",
    about = "Searches Pokémon Showdown battle logs"
)]
struct Options {
    #[structopt(
        short = "w",
        long = "wins-only",
        help = "Only display games where the searched user wins"
    )]
    wins_only: bool,

    #[structopt(
        short = "f",
        long = "forfeits-only",
        help = "Only display games that end with one player forfeiting"
    )]
    forfeits_only: bool,

    #[structopt(
        short = "j",
        long = "threads",
        help = "The number of threads to spawn",
        default_value = "2"
    )]
    threads: u32,

    #[structopt(help = "The username whose battles will be displayed")]
    username: String,

    #[structopt(
        help = "The directories to search for battle logs in. Searches recursively.",
        required(true)
    )]
    #[structopt(parse(from_os_str))]
    directories: Vec<PathBuf>,
}

fn main() -> Result<(), BattleSearchError> {
    let options = Options::from_args();
    let mut senders = vec![];
    let mut join_handles = vec![];
    for _ in 1..=options.threads {
        let (sender, receiver) = mpsc::channel();
        let username = options.username.clone();
        let wins_only = options.wins_only;
        let forfeits_only = options.forfeits_only;
        join_handles.push(thread::spawn(move || {
            let mut searcher =
                BattleSearcher::new(&username, PIKKR_TRAINING_ROUNDS, wins_only, forfeits_only);
            loop {
                match receiver.recv() {
                    Ok(data) => match data {
                        ToSend::File(path, date) => {
                            if let Err(e) = searcher.check_log(&path, &date) {
                                eprintln!("Error parsing {:?}: {:?}", path, e);
                            }
                        }
                        ToSend::Done => return,
                    },
                    Err(e) => {
                        eprintln!("{:?}", e);
                        return;
                    }
                }
            }
        }));
        senders.push(sender);
    }

    for directory in &(options.directories) {
        handle_dir(directory, &senders)?;
    }

    for sender in senders {
        sender.send(ToSend::Done)?;
    }

    for handle in join_handles {
        handle.join()?;
    }

    Ok(())
}
