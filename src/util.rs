use log::debug;
use std::path::Path;

pub(crate) fn extract_java_version<E>(
    mut lines: impl Iterator<Item = Result<String, E>>,
) -> Result<Option<String>, E> {
    lines
        .find_map(|line_result| -> Option<Result<String, E>> {
            let line = match line_result {
                Ok(l) => l,
                Err(e) => return Some(Err(e)),
            };
            debug!("{}", line);
            let trimmed = line.trim();
            if !trimmed.starts_with("JAVA_VERSION") {
                return None;
            }

            let index = match trimmed.find('=') {
                None => return None,
                Some(i) => i,
            };

            let mut value = &trimmed[(index + 1)..];
            if value.starts_with('"') {
                value = &value[1..];
                if value.ends_with('"') {
                    value = &value[..value.len() - 1];
                }
            }

            Some(Ok(value.to_string()))
        })
        .transpose()
}

pub(crate) fn is_symlink<P: AsRef<Path>>(symlink: P) -> bool {
    // This is a copy of std::path::Path::is_symlink()
    // currently marked unstable, but the parts that make up the function aren't
    return symlink
        .as_ref()
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
}
