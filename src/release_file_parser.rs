use log::debug;

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
