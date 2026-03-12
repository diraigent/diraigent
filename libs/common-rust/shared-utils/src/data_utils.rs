use csv::ReaderBuilder;
use std::error::Error;
use std::fs::File;

pub trait FromRecord: Sized {
    fn from_record(record: &csv::StringRecord) -> Result<Self, Box<dyn Error>>;
}

pub fn read_from_csv<T: FromRecord>(path: &str) -> Result<Vec<T>, Box<dyn Error>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let full_path = std::path::Path::new(manifest_dir).join(path);

    let file = File::open(full_path)?;
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(file);

    rdr.records()
        .map(|result| {
            let record = result?;
            T::from_record(&record)
        })
        .collect()
}

pub fn build_map_by_key<K, V, F>(items: &[V], mut key_fn: F) -> std::collections::HashMap<K, V>
where
    K: std::hash::Hash + Eq,
    V: Clone,
    F: FnMut(&V) -> K,
{
    items.iter().map(|v| (key_fn(v), v.clone())).collect()
}
