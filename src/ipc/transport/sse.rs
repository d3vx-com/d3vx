//! Server-Sent Events Transport
//!
//! SSE transport for server-to-client streaming.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, warn};

use super::traits::StreamingTransport;
use super::{Transport, TransportError, TransportEvent};

pub struct SseTransport {
    client: Client,
    url: String,
    headers: std::collections::HashMap<String, String>,
    connected: Arc<RwLock<bool>>,
    receiver: Arc<RwLock<Option<mpsc::Receiver<TransportEvent<serde_json::Value>>>>>,
}

impl SseTransport {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            url: url.into(),
            headers: std::collections::HashMap::new(),
            connected: Arc::new(RwLock::new(false)),
            receiver: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_auth(mut self, token: impl Into<String>) -> Self {
        self.headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", token.into()),
        );
        self
    }

    async fn start_streaming(&self) -> Result<(), TransportError> {
        let mut req = self.client.get(&self.url);
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }
        req = req.header("Accept", "text/event-stream");

        let response = req
            .send()
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))?;
        if !response.status().is_success() {
            return Err(TransportError::Connection(response.status().to_string()));
        }

        let (tx, rx) = mpsc::channel(100);
        let mut stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = Vec::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        for b in bytes {
                            if b == b'\n' {
                                let line_data: Vec<u8> = buffer.clone();
                                buffer.clear();
                                let line = String::from_utf8_lossy(&line_data);

                                if line.starts_with("data:") {
                                    let data = line.trim_start_matches("data:").trim();
                                    if data.is_empty() {
                                        continue;
                                    }
                                    if data == "[DONE]" {
                                        let _ = tx.send(TransportEvent::Disconnected).await;
                                        break;
                                    }
                                    if let Ok(json) =
                                        serde_json::from_str::<serde_json::Value>(data)
                                    {
                                        let _ = tx.send(TransportEvent::Message(json)).await;
                                    }
                                }
                            } else {
                                buffer.push(b);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "SSE stream error");
                        let _ = tx
                            .send(TransportEvent::Error(TransportError::Receive(
                                e.to_string(),
                            )))
                            .await;
                        break;
                    }
                }
            }
        });

        *self.receiver.write().await = Some(rx);
        Ok(())
    }
}

#[async_trait]
impl Transport for SseTransport {
    type Message = serde_json::Value;

    async fn connect(&mut self) -> Result<(), TransportError> {
        self.start_streaming().await?;
        *self.connected.write().await = true;
        debug!(url = %self.url, "SSE transport connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        *self.connected.write().await = false;
        *self.receiver.write().await = None;
        Ok(())
    }

    async fn send(&self, _msg: &Self::Message) -> Result<(), TransportError> {
        if !self.is_connected() {
            return Err(TransportError::NotConnected);
        }
        Ok(())
    }

    async fn receive(&self) -> Result<Self::Message, TransportError> {
        Err(TransportError::Receive(
            "Use subscribe() for SSE".to_string(),
        ))
    }

    fn is_connected(&self) -> bool {
        self.connected.try_read().map(|g| *g).unwrap_or(false)
    }
}

#[async_trait]
impl StreamingTransport for SseTransport {
    async fn send_streaming(&self, _msg: &Self::Message) -> Result<(), TransportError> {
        Ok(())
    }

    fn subscribe(&self) -> mpsc::Receiver<TransportEvent<Self::Message>> {
        let (tx, rx) = mpsc::channel(100);
        let receiver = self.receiver.clone();

        tokio::spawn(async move {
            if let Some(mut rx) = receiver.write().await.take() {
                while let Some(event) = rx.recv().await {
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
        });

        rx
    }
}
