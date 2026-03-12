use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::data_utils::{FromRecord, build_map_by_key, read_from_csv};

#[derive(Clone, Deserialize, Serialize)]
pub struct Language {
    pub name: String,
    pub iso6391: String,  // 2-letter code
    pub iso6392: String,  // 3-letter code (bibliographic)
    pub iso6392b: String, // 3-letter code (terminologic)
}

static LANGUAGES: Lazy<Vec<Language>> =
    Lazy::new(|| read_from_csv("data/language.csv").unwrap_or_default());

static LANGUAGES_BY_ISO2: Lazy<HashMap<String, Language>> =
    Lazy::new(|| build_map_by_key(&LANGUAGES, |l| l.iso6391.clone()));

static LANGUAGES_BY_ISO3: Lazy<HashMap<String, Language>> =
    Lazy::new(|| build_map_by_key(&LANGUAGES, |l| l.iso6392.clone()));

static LANGUAGES_BY_ISO3B: Lazy<HashMap<String, Language>> =
    Lazy::new(|| build_map_by_key(&LANGUAGES, |l| l.iso6392b.clone()));

impl FromRecord for Language {
    fn from_record(record: &csv::StringRecord) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Language {
            name: record[0].to_string(),
            iso6391: record[1].to_string(),
            iso6392: record[2].to_string(),
            iso6392b: record[3].to_string(),
        })
    }
}

pub fn find_language(code: &str) -> Option<Language> {
    let lowercase_code = code.to_lowercase();
    match code.len() {
        2 => LANGUAGES_BY_ISO2.get(&lowercase_code).cloned(),
        3 => LANGUAGES_BY_ISO3
            .get(&lowercase_code)
            .cloned()
            .or_else(|| LANGUAGES_BY_ISO3B.get(&lowercase_code).cloned()),
        _ => LANGUAGES
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(code))
            .cloned(),
    }
}
