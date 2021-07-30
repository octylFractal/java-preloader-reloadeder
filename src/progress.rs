use console::style;
use indicatif::{ProgressBar, ProgressStyle};

pub fn new_progress_bar(bar_length: Option<u64>) -> ProgressBar {
    let bar_style = match bar_length {
        Some(_) => ProgressStyle::default_bar()
            .template(
                "{percent:>3}%[{bar:60.cyan/blue}] {bytes:>8}/{total_bytes} {bytes_per_sec} {wide_msg}",
            )
            .progress_chars("#|-"),
        None => ProgressStyle::default_spinner()
            .template(
                &*format!("{}{}{}", "    [", style("-".repeat(60)).for_stderr().blue(), "] {bytes:>8} {bytes_per_sec} {wide_msg}")
            ),
    };

    ProgressBar::new(bar_length.unwrap_or(!0)).with_style(bar_style)
}
