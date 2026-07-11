use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub(crate) fn read_first_json_line(path: &Path) -> Option<serde_json::Value> {
    let file = File::open(path).ok()?;
    let first_line = BufReader::new(file).lines().next()?.ok()?;
    serde_json::from_str(&first_line).ok()
}

pub(crate) fn json_str<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str()
}
