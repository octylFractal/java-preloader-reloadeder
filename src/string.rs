pub trait SplittingExt {
    fn split_optional(&self, delimiter: char) -> (&str, Option<&str>);
}

impl SplittingExt for str {
    fn split_optional(&self, delimiter: char) -> (&str, Option<&str>) {
        match self.split_once(delimiter) {
            Some((first, second)) => (first, Some(second)),
            None => (self, None),
        }
    }
}
