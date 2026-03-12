use crate::data_utils::{FromRecord, build_map_by_key, read_from_csv};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::error::Error;

#[derive(Clone)]
pub struct Country {
    pub num: i32,
    pub name: String,
    pub iso31661a2: String,
    pub iso31661a3: String,
}

static COUNTRIES: Lazy<Vec<Country>> =
    Lazy::new(|| read_countries_from_csv("data/country.csv").unwrap_or_default());

static COUNTRIES_BY_ISO2: Lazy<HashMap<String, Country>> =
    Lazy::new(|| build_map_by_key(&COUNTRIES, |c| c.iso31661a2.clone()));

static COUNTRIES_BY_ISO3: Lazy<HashMap<String, Country>> =
    Lazy::new(|| build_map_by_key(&COUNTRIES, |c| c.iso31661a3.clone()));

static COUNTRIES_BY_NUM: Lazy<HashMap<i32, Country>> =
    Lazy::new(|| build_map_by_key(&COUNTRIES, |c| c.num));

fn read_countries_from_csv(path: &str) -> Result<Vec<Country>, Box<dyn Error>> {
    read_from_csv(path)
}

impl FromRecord for Country {
    fn from_record(record: &csv::StringRecord) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Country {
            name: record[0].to_string(),
            iso31661a2: record[1].to_string(),
            iso31661a3: record[2].to_string(),
            num: record[3].parse()?,
        })
    }
}
pub fn find_country(identifier: &str) -> Option<Country> {
    // Try parsing as number first (most specific)
    if let Ok(num) = identifier.parse::<i32>()
        && let Some(country) = COUNTRIES_BY_NUM.get(&num)
    {
        return Some(country.clone());
    }

    // Try ISO codes (fixed length, fast HashMap lookup)
    match identifier.len() {
        2 => COUNTRIES_BY_ISO2.get(&identifier.to_lowercase()).cloned(),
        3 => COUNTRIES_BY_ISO3.get(&identifier.to_lowercase()).cloned(),
        _ => {
            // Fall back to name search (case-insensitive)
            COUNTRIES
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(identifier))
                .cloned()
        }
    }
}

pub fn find_by_iso31661(code: &str) -> Option<Country> {
    COUNTRIES_BY_ISO2.get(code).cloned()
}

pub fn find_by_iso31662(code: &str) -> Option<Country> {
    COUNTRIES_BY_ISO3.get(code).cloned()
}

pub fn find_by_num(num: i32) -> Option<Country> {
    COUNTRIES_BY_NUM.get(&num).cloned()
}

pub fn find_by_name(name: &str) -> Option<Country> {
    COUNTRIES
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(name))
        .cloned()
}
