//! SDK Mode Handler
//!
//! Non-interactive SDK mode for programmatic d3vx usage.

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, info};

use super::events::{ControlRequest, ControlResponse, SdkEvent, SdkResponse};

#[derive(Debug, Clone)]
pub struct SdkOptions {
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub event_tx: Option<mpsc::Sender<SdkEvent>>,
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)] // Reserved for SDK permission modes
pub enum PermissionMode {
    AcceptAll,
    Ask,
    #[default]
    Bypass,
}

pub struct SdkMode {
    reader: BufReader<tokio::io::Stdin>,
    #[allow(dead_code)] // Reserved for SDK event streaming
    event_tx: mpsc::Sender<SdkEvent>,
}

impl SdkMode {
    pub async fn new(_options: SdkOptions) -> Result<Self> {
        info!("Starting SDK mode");
        Ok(Self {
            reader: BufReader::new(tokio::io::stdin()),
            event_tx: mpsc::channel(100).0,
        })
    }

    pub fn event_receiver(&self) -> mpsc::Receiver<SdkEvent> {
        let (tx, rx) = mpsc::channel(100);
        tokio::spawn(async move {
            let _ = tx;
        });
        rx
    }

    pub async fn handle_control(&self, request: ControlRequest) -> Result<ControlResponse> {
        debug!(request = ?request, "Handling control request");
        match request {
            ControlRequest::Initialize { model } => {
                info!(model = ?model, "SDK initialized");
                Ok(ControlResponse::Initialized {
                    session_id: uuid::Uuid::new_v4().to_string(),
                })
            }
            ControlRequest::SetModel { model } => {
                info!(model = %model, "Model changed");
                Ok(ControlResponse::ModelChanged { model })
            }
            ControlRequest::SetPermissionMode { mode: _ } => {
                debug!("Permission mode set");
                Ok(ControlResponse::Initialized {
                    session_id: uuid::Uuid::new_v4().to_string(),
                })
            }
            ControlRequest::Interrupt => {
                info!("Session interrupted");
                Ok(ControlResponse::Interrupted)
            }
            ControlRequest::Resume => {
                info!("Session resumed");
                Ok(ControlResponse::Resumed)
            }
            ControlRequest::SetMaxThinkingTokens { tokens: _ } => {
                debug!("Max thinking tokens set");
                Ok(ControlResponse::Initialized {
                    session_id: uuid::Uuid::new_v4().to_string(),
                })
            }
        }
    }

    pub async fn read_input(&mut self) -> Result<Option<SdkResponse>> {
        let mut line = String::new();
        match self.reader.read_line(&mut line).await {
            Ok(0) => Ok(None),
            Ok(_) => {
                let json: SdkResponse = serde_json::from_str(line.trim())?;
                Ok(Some(json))
            }
            Err(_) => Ok(None),
        }
    }
}
