//! HTTP Transport
//!
//! Simple HTTP POST transport for request/response patterns.

use async_trait::async_trait;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::traits::StreamingTransport;
use super::{Transport, TransportError, TransportEvent};

#[derive(Debug)]
pub struct HttpTransport {
    client: Client,
    url: String,
    headers: std::collections::HashMap<String, String>,
    connected: Arc<RwLock<bool>>,
}

impl HttpTransport {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            url: url.into(),
            headers: std::collections::HashMap::new(),
            connected: Arc::new(RwLock::new(false)),
        }
    }

    pub fn with_auth(mut self, token: impl Into<String>) -> Self {
        self.headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", token.into()),
        );
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

#[async_trait]
impl Transport for HttpTransport {
    type Message = serde_json::Value;

    async fn connect(&mut self) -> Result<(), TransportError> {
        *self.connected.write().await = true;
        debug!(url = %self.url, "HTTP transport connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        *self.connected.write().await = false;
        Ok(())
    }

    async fn send(&self, msg: &Self::Message) -> Result<(), TransportError> {
        if !self.is_connected() {
            return Err(TransportError::NotConnected);
        }

        let mut req = self.client.post(&self.url);
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }

        match req.json(msg).send().await {
            Ok(resp) if resp.status().is_success() => {
                debug!("HTTP request successful");
                Ok(())
            }
            Ok(resp) => {
                warn!(status = %resp.status(), "HTTP request failed");
                Err(TransportError::Send(resp.status().to_string()))
            }
            Err(e) => Err(TransportError::Send(e.to_string())),
        }
    }

    async fn receive(&self) -> Result<Self::Message, TransportError> {
        Err(TransportError::Receive(
            "HTTP transport is request/response only".to_string(),
        ))
    }

    fn is_connected(&self) -> bool {
        // Safe read without async
        self.connected.try_read().map(|g| *g).unwrap_or(false)
    }
}

#[async_trait]
impl StreamingTransport for HttpTransport {
    async fn send_streaming(&self, msg: &Self::Message) -> Result<(), TransportError> {
        self.send(msg).await
    }

    fn subscribe(&self) -> tokio::sync::mpsc::Receiver<TransportEvent<Self::Message>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        tokio::spawn(async move {
            let _ = tx
                .send(TransportEvent::Error(TransportError::Receive(
                    "HTTP transport does not support streaming".to_string(),
                )))
                .await;
        });
        rx
    }
}
