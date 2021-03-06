use lazy_static::*;
use regex::Regex;
/// Battlesearch code for Pokémon Showdown battle logs
use std::{any::Any, fs, path::PathBuf};

#[derive(Debug)]
pub enum BattleSearchError {
    FaultyJSON(String),
    Path(String),
    IO(std::io::Error),
    Thread(std::sync::mpsc::SendError<ToSend>),
    Join(Box<dyn Any + Send>),
}

pub enum ToSend {
    File(PathBuf, String),
    Done,
}

impl From<std::io::Error> for BattleSearchError {
    fn from(err: std::io::Error) -> Self {
        BattleSearchError::IO(err)
    }
}
impl From<std::sync::mpsc::SendError<ToSend>> for BattleSearchError {
    fn from(err: std::sync::mpsc::SendError<ToSend>) -> Self {
        BattleSearchError::Thread(err)
    }
}
impl From<Box<dyn Any + Send>> for BattleSearchError {
    fn from(err: Box<dyn Any + Send>) -> Self {
        BattleSearchError::Join(err)
    }
}

lazy_static! {
    static ref ID_REGEX: Regex = Regex::new(r"[^A-Za-z0-9]").unwrap();
}

// Taken from https://github.com/AnnikaCodes/anonbattle/blob/main/src/anonymizer.rs#L36
// Perhaps I should share code somehow in the future; perhaps with a battle-tools library crate?
fn str_to_id(str: &str) -> String {
    (*ID_REGEX.replace_all(str, "")).to_lowercase()
}

fn bytes_to_id(bytes: &Option<&[u8]>) -> Option<String> {
    match bytes {
        Some(b) => Some(str_to_id(&String::from_utf8_lossy(b))),
        None => None,
    }
}

pub struct BattleSearcher<'a> {
    user_id: String,
    json_parser: pikkr_annika::Pikkr<'a>,
    wins_only: bool,
    forfeits_only: bool,
}

impl<'a> BattleSearcher<'a> {
    pub fn new(
        username: &str,
        pikkr_training_rounds: usize,
        wins_only: bool,
        forfeits_only: bool,
    ) -> Self {
        let json_parser = pikkr_annika::Pikkr::new(
            &vec![
                "$.p1".as_bytes(),      // p1 name - idx 0
                "$.p2".as_bytes(),      // p2 name - idx 1
                "$.winner".as_bytes(),  // winner - idx 2
                "$.endType".as_bytes(), // end type - idx 3
            ],
            pikkr_training_rounds,
        )
        .unwrap();

        Self {
            user_id: str_to_id(username),
            json_parser,
            wins_only,
            forfeits_only,
        }
    }

    /// json is in the form [p1name, p2name, winner, endType]
    pub fn check_log(&mut self, path: &PathBuf, date: &str) -> Result<(), BattleSearchError> {
        let data = fs::read(path)?;
        let json = self.json_parser.parse(&data).unwrap();

        if json.len() != 4 {
            // should never happen
            return Err(BattleSearchError::FaultyJSON(format!(
                "BattleSearcher::check_log(): found {} elements in parsed JSON (expected 4)",
                json.len()
            )));
        }

        // parse players
        let p1id = match bytes_to_id(json.get(0).unwrap()) {
            Some(a) => a,
            None => return Err(BattleSearchError::FaultyJSON(format!("No p1 value"))),
        };
        let p2id = match bytes_to_id(json.get(1).unwrap()) {
            Some(a) => a,
            None => return Err(BattleSearchError::FaultyJSON(format!("No p2 value"))),
        };
        let p1_is_searched_user = p1id == self.user_id;
        let p2_is_searched_user = p2id == self.user_id;
        if !p1_is_searched_user && !p2_is_searched_user {
            // Searched user is not a player in the battle.
            return Ok(());
        }

        // parse winner
        let winner_id = bytes_to_id(json.get(2).unwrap());
        let searched_user_won = match winner_id {
            Some(ref winner) => winner == &self.user_id,
            None => false,
        };
        if !searched_user_won && self.wins_only {
            return Ok(());
        }

        // parse endType
        let is_forfeit = match json.get(3).unwrap() {
            Some(bytes) => String::from_utf8_lossy(bytes) == "\"forfeit\"",
            None => false,
        };
        if !is_forfeit && self.forfeits_only {
            return Ok(());
        }

        // formatting
        let win_type_str = if is_forfeit { "by forfeit" } else { "normally" };
        let win_str = match winner_id {
            Some(ref winner) => format!("{} won {}", winner, win_type_str),
            None => String::from("there was no winner"),
        };

        let room = match path.file_name() {
            Some(os_str) => String::from(os_str.to_str().unwrap_or("unknown file")),
            None => String::from("unknown file"),
        }
        .replace(".log.json", "");

        println!(
            "({}) <<{}>> {} vs. {} ({})",
            date, room, p1id, p2id, win_str
        );

        Ok(())
    }
}
