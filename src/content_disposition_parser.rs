use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;

const FILENAME: &str = "filename=";
const FILENAME_EXT: &str = "filename*=";

pub fn parse_filename(header: &str) -> Result<String> {
    header.split(";")
        .filter_map(|x| {
            let trimmed = x.trim();
            if !trimmed.starts_with("filename") {
                return None;
            }
            let actual_filename = if trimmed.starts_with(FILENAME) {
                let result = parse_filename_value(&trimmed[FILENAME.len()..]);
                if result.is_err() {
                    return Some(result);
                }
                result.unwrap()
            } else if trimmed.starts_with(FILENAME_EXT) {
                return Some(Err(
                    anyhow!("Writing code for filename*= is hard! I'll do it now if you tell me about this.")
                ));
            } else {
                return None;
            };
            Some(Ok(actual_filename))
        })
        .next()
        .ok_or_else(|| anyhow!("no filename in header"))
        .and_then(|e| e)
}

static TOKEN_RE: Lazy<Regex> = Lazy::new(|| Regex::new("^[!#$%&'*+.0-9A-Z^_`a-z|~-]+$").unwrap());
static QUOTED_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^([^\x00-\x1F\x7F\"]|[\r\n\t ]|\\\")+$").unwrap());

// A REALLY BAD content-disposition parser
// Didn't need to work very well so it is likely to break / accept invalid filenames
// Report and I'll add support for it.
fn parse_filename_value(value: &str) -> Result<String> {
    if TOKEN_RE.is_match(value) {
        return Ok(value.to_string());
    }
    if !value.starts_with('"') || !value.ends_with('"') {
        return Err(anyhow!("Filename didn't match TOKEN and isn't quoted"));
    }
    let trimmed = &value[1..value.len()];
    if !QUOTED_TEXT_RE.is_match(trimmed) {
        return Err(anyhow!("Quoted filename doesn't match TEXT regex"));
    }
    return Ok(value.replace("\\\"", "\""));
}
