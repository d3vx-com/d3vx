//! Transport Trait Definitions
//!
//! Core abstraction for communication transports.

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Send failed: {0}")]
    Send(String),
    #[error("Receive failed: {0}")]
    Receive(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Not connected")]
    NotConnected,
}

#[derive(Debug, Clone)]
pub enum TransportEvent<T> {
    Connected,
    Disconnected,
    Message(T),
    Error(TransportError),
}

#[async_trait]
pub trait Transport: Send + Sync {
    type Message: Serialize + DeserializeOwned + Send + 'static;

    async fn connect(&mut self) -> Result<(), TransportError>;
    async fn disconnect(&mut self) -> Result<(), TransportError>;
    async fn send(&self, msg: &Self::Message) -> Result<(), TransportError>;
    async fn receive(&self) -> Result<Self::Message, TransportError>;
    fn is_connected(&self) -> bool;
}

#[async_trait]
#[allow(dead_code)] // Reserved for streaming transports (SSE/WebSocket)
pub trait StreamingTransport: Transport {
    async fn send_streaming(&self, msg: &Self::Message) -> Result<(), TransportError>;
    fn subscribe(&self) -> tokio::sync::mpsc::Receiver<TransportEvent<Self::Message>>;
}
