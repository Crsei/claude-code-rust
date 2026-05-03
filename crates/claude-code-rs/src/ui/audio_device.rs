//! Audio-device compatibility surface for the Codex TUI layout.
//!
//! cc-rust currently routes voice capture through `crate::voice`, whose default
//! backend is intentionally unsupported. This module keeps the Codex-shaped
//! `audio_device` boundary available to UI code while reporting that runtime
//! truth explicitly.

use crate::voice::audio::{AudioCaptureBackend, NullAudioBackend};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeAudioDeviceKind {
    Input,
    Output,
}

impl RealtimeAudioDeviceKind {
    pub fn noun(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub kind: RealtimeAudioDeviceKind,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSupportStatus {
    pub backend: String,
    pub available: bool,
    pub reason: Option<String>,
}

pub fn audio_support_status() -> AudioSupportStatus {
    let backend = NullAudioBackend::new();
    match backend.is_available() {
        Ok(()) => AudioSupportStatus {
            backend: backend.name().to_string(),
            available: true,
            reason: None,
        },
        Err(err) => AudioSupportStatus {
            backend: backend.name().to_string(),
            available: false,
            reason: Some(err.reason().to_string()),
        },
    }
}

pub fn list_realtime_audio_device_names(
    kind: RealtimeAudioDeviceKind,
) -> Result<Vec<String>, String> {
    let status = audio_support_status();
    if status.available {
        Ok(Vec::new())
    } else {
        Err(format!(
            "Failed to load realtime {} devices: {}",
            kind.noun(),
            status
                .reason
                .unwrap_or_else(|| "audio is unavailable".to_string())
        ))
    }
}

pub fn list_realtime_audio_devices(
    kind: RealtimeAudioDeviceKind,
) -> Result<Vec<AudioDeviceInfo>, String> {
    list_realtime_audio_device_names(kind).map(|names| {
        names
            .into_iter()
            .enumerate()
            .map(|(idx, name)| AudioDeviceInfo {
                name,
                kind,
                is_default: idx == 0,
            })
            .collect()
    })
}

pub fn voice_capture_backend_name() -> &'static str {
    NullAudioBackend::new().name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_null_backend_unavailable() {
        let status = audio_support_status();
        assert_eq!(status.backend, "null");
        assert!(!status.available);
        assert!(status.reason.unwrap().contains("unsupported"));
    }
}
