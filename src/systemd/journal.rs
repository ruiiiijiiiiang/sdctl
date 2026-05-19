use std::io::{Error, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::LinesStream;

use tailspin::Highlighter;

pub struct JournalManager {
    highlighter: Highlighter,
}

impl JournalManager {
    pub fn new() -> Self {
        Self {
            highlighter: Highlighter::default(),
        }
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
            .stderr(std::process::Stdio::null());

        let output = command.output().await?;

        if !output.status.success() {
            return Err(Error::other(format!(
                "journalctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let content = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<String> = content
            .lines()
            .map(|line| self.highlighter.apply(line).into_owned())
            .collect();

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
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut child = command.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::other("Failed to capture stdout"))?;

        let reader = BufReader::new(stdout);
        let highlighter = self.highlighter;

        Ok(LinesStream::new(reader.lines()).map(move |line| {
            let line = line.unwrap_or_default();
            highlighter.apply(&line).into_owned()
        }))
    }
}
