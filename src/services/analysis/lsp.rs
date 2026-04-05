use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};

pub struct LspClient {
    _child: Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl LspClient {
    pub fn new(command: &str, args: &[&str]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        Ok(Self {
            _child: child,
            stdin,
            stdout,
        })
    }

    pub fn send_request(&mut self, method: &str, params: Value) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let body = serde_json::to_string(&request)?;
        let content = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        self.stdin.write_all(content.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    pub fn read_response(&mut self) -> Result<Value> {
        let mut line = String::new();
        let mut content_length = 0;

        while self.stdout.read_line(&mut line)? > 0 {
            if line == "\r\n" {
                break;
            }
            if line.starts_with("Content-Length: ") {
                content_length = line
                    .trim_start_matches("Content-Length: ")
                    .trim()
                    .parse::<usize>()?;
            }
            line.clear();
        }

        let mut body = vec![0u8; content_length];
        self.stdout.read_exact(&mut body)?;

        let response: Value = serde_json::from_slice(&body)?;
        Ok(response)
    }
}
