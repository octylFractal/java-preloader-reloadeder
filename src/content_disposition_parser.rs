use regex::Regex;
use std::error::Error;

const FILENAME: &str = "filename=";
const FILENAME_EXT: &str = "filename*=";

pub fn parse_filename(header: &str) -> Result<String, Box<dyn Error>> {
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
                    Box::from("Writing code for filename*= is hard! I'll do it now if you tell me about this.")
                ));
            } else {
                return None;
            };
            Some(Ok(actual_filename))
        })
        .next()
        .ok_or(Box::from("no filename in header"))
        .and_then(|e| e)
}
lazy_static! {
    static ref TOKEN_RE: Regex = Regex::new("^[!#$%&'*+.0-9A-Z^_`a-z|~-]+$").unwrap();
    static ref QUOTED_TEXT_RE: Regex =
        Regex::new("^([^\x00-\x1F\x7F\"]|[\r\n\t ]|\\\")+$").unwrap();
}

// A REALLY BAD content-disposition parser
// Didn't need to work very well so it is likely to break / accept invalid filenames
// Report and I'll add support for it.
fn parse_filename_value(value: &str) -> Result<String, Box<dyn Error>> {
    if TOKEN_RE.is_match(value) {
        return Ok(value.to_string());
    }
    if !value.starts_with('"') || !value.ends_with('"') {
        return Err(Box::from("Filename didn't match TOKEN and isn't quoted"));
    }
    let trimmed = &value[1..value.len()];
    if !QUOTED_TEXT_RE.is_match(trimmed) {
        return Err(Box::from("Quoted filename doesn't match TEXT regex"));
    }
    return Ok(value.replace("\\\"", "\""));
}
