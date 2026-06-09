use std::{
    io::{Error, Result},
    process::Stdio,
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use tokio_stream::{StreamExt, wrappers::LinesStream};

pub struct JournalManager;

impl JournalManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch_logs(
        &self,
        unit_name: &str,
        scope: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        let mut command = Command::new("journalctl");
        if scope == "session" {
            command.arg("--user");
        }
        command
            .arg("-u")
            .arg(unit_name)
            .arg("-n")
            .arg(limit.to_string())
            .arg("--no-pager")
            .stderr(Stdio::null());

        let output = command.output().await?;

        if !output.status.success() {
            return Err(Error::other(format!(
                "journalctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let content = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<String> = content.lines().map(String::from).collect();

        Ok(lines)
    }

    pub async fn follow_logs(
        self,
        unit_name: &str,
        scope: &str,
        limit: usize,
    ) -> Result<impl tokio_stream::Stream<Item = String>> {
        let mut command = Command::new("journalctl");
        if scope == "session" {
            command.arg("--user");
        }
        command
            .arg("-u")
            .arg(unit_name)
            .arg("-n")
            .arg(limit.to_string())
            .arg("-f")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = command.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::other("Failed to capture stdout"))?;

        let reader = BufReader::new(stdout);

        Ok(LinesStream::new(reader.lines()).map(move |line| line.unwrap_or_default()))
    }
}
