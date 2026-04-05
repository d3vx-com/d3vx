//! NDJSON (Newline Delimited JSON) Utilities
//!
//! Reader and writer for streaming JSON lines.

use anyhow::{Context, Result};
use futures::Stream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct NdJsonReader<R> {
    reader: BufReader<R>,
}

impl<R: tokio::io::AsyncRead + Unpin> NdJsonReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
        }
    }

    pub async fn read_line(&mut self) -> Result<Option<serde_json::Value>> {
        let mut line = String::new();
        match self
            .reader
            .read_line(&mut line)
            .await
            .context("Failed to read line")
        {
            Ok(0) => Ok(None),
            Ok(_) => {
                let json =
                    serde_json::from_str(line.trim()).context("Failed to parse JSON line")?;
                Ok(Some(json))
            }
            Err(e) => Err(anyhow::anyhow!("Read error: {}", e)),
        }
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<serde_json::Value>> {
        futures::stream::unfold(self, |mut reader| async move {
            match reader.read_line().await {
                Ok(Some(v)) => Some((Ok(v), reader)),
                Ok(None) => None,
                Err(e) => Some((Err(e), reader)),
            }
        })
    }
}

pub struct NdJsonWriter<W> {
    writer: tokio::io::BufWriter<W>,
}

impl<W: tokio::io::AsyncWrite + Unpin> NdJsonWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: tokio::io::BufWriter::new(writer),
        }
    }

    pub async fn write_line(&mut self, value: &serde_json::Value) -> Result<()> {
        let line = serde_json::to_string(value).context("Failed to serialize JSON")?;
        let line = format!("{}\n", line);
        self.writer
            .write_all(line.as_bytes())
            .await
            .context("Failed to write")?;
        self.writer.flush().await.context("Failed to flush")?;
        Ok(())
    }

    pub async fn write_batch(&mut self, values: &[serde_json::Value]) -> Result<()> {
        for value in values {
            self.write_line(value).await?;
        }
        Ok(())
    }
}
