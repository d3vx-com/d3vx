//! WebSocket Transport
//!
//! WebSocket transport for bidirectional real-time communication.

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, warn};
use url::Url;

use super::traits::StreamingTransport;
use super::{Transport, TransportError, TransportEvent};

pub struct WebSocketTransport {
    url: String,
    connected: Arc<RwLock<bool>>,
    sender: Arc<RwLock<Option<futures::channel::mpsc::Sender<Message>>>>,
    receiver: Arc<RwLock<Option<mpsc::Receiver<TransportEvent<serde_json::Value>>>>>,
}

impl WebSocketTransport {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            connected: Arc::new(RwLock::new(false)),
            sender: Arc::new(RwLock::new(None)),
            receiver: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_auth(mut self, token: impl Into<String>) -> Self {
        let url = format!("{}?token={}", self.url, token.into());
        Self { url, ..self }
    }

    async fn establish_connection(&self) -> Result<(), TransportError> {
        let (ws_stream, _) = connect_async(&self.url)
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))?;

        let (write, read) = ws_stream.split();
        let (tx, rx) = futures::channel::mpsc::unbounded();
        let (event_tx, event_rx) = mpsc::channel(100);

        write.send(Message::Text("".to_string())).await.ok();

        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let mut write = write;
            let mut rx = rx;

            while let Some(msg) = rx.next().await {
                if write.send(msg).await.is_err() {
                    break;
                }
            }
            let _ = event_tx_clone.send(TransportEvent::Disconnected).await;
        });

        tokio::spawn(async move {
            let mut read = read;

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            let _ = event_tx.send(TransportEvent::Message(json)).await;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        let _ = event_tx.send(TransportEvent::Disconnected).await;
                        break;
                    }
                    Err(e) => {
                        warn!(error = %e, "WebSocket read error");
                        let _ = event_tx
                            .send(TransportEvent::Error(TransportError::Receive(
                                e.to_string(),
                            )))
                            .await;
                    }
                    _ => {}
                }
            }
        });

        *self.sender.write().await = Some(tx);
        *self.receiver.write().await = Some(event_rx);
        *self.connected.write().await = true;
        Ok(())
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    type Message = serde_json::Value;

    async fn connect(&mut self) -> Result<(), TransportError> {
        self.establish_connection().await?;
        debug!(url = %self.url, "WebSocket connected");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        if let Some(tx) = self.sender.write().await.take() {
            let _ = tx.close().await;
        }
        *self.connected.write().await = false;
        Ok(())
    }

    async fn send(&self, msg: &Self::Message) -> Result<(), TransportError> {
        let sender = self.sender.read().await;
        let sender = sender.as_ref().ok_or(TransportError::NotConnected)?;
        let text = serde_json::to_string(msg).map_err(|e| TransportError::Send(e.to_string()))?;
        sender
            .unbounded_send(Message::Text(text))
            .map_err(|e| TransportError::Send(e.to_string()))?;
        Ok(())
    }

    async fn receive(&self) -> Result<Self::Message, TransportError> {
        Err(TransportError::Receive(
            "Use subscribe() for WebSocket".to_string(),
        ))
    }

    fn is_connected(&self) -> bool {
        self.connected.try_read().map(|g| *g).unwrap_or(false)
    }
}

#[async_trait]
impl StreamingTransport for WebSocketTransport {
    async fn send_streaming(&self, msg: &Self::Message) -> Result<(), TransportError> {
        self.send(msg).await
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
