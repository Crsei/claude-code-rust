//! Typed sender wrapper for app events.

use tokio::sync::mpsc;

use super::app_event::AppEvent;

#[derive(Debug, Clone)]
pub struct AppEventSender {
    tx: mpsc::UnboundedSender<AppEvent>,
}

impl AppEventSender {
    pub fn new(tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        Self { tx }
    }

    pub fn send(&self, event: AppEvent) -> Result<(), mpsc::error::SendError<AppEvent>> {
        self.tx.send(event)
    }

    pub fn notice(
        &self,
        message: impl Into<String>,
    ) -> Result<(), mpsc::error::SendError<AppEvent>> {
        self.send(AppEvent::LocalNotice {
            message: message.into(),
        })
    }
}

pub fn channel() -> (AppEventSender, mpsc::UnboundedReceiver<AppEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (AppEventSender::new(tx), rx)
}
