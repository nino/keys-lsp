use anyhow::Result;
use std::fs;
use std::io::Write;

const LOG_FILE: &str = "/Users/Nino/log.txt";

fn log_fallible(message: &str) -> Result<()> {
    let mut log_file = fs::File::options().append(true).open(LOG_FILE)?;
    let formatted_message = format!("{}: {}\n", chrono::Local::now(), message);
    let _ = log_file.write_all(formatted_message.as_bytes());
    Ok(())
}

pub fn log(message: &str) {
    let _ = log_fallible(message);
}
