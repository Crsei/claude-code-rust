//! Shared status icon/label primitive for compact text status renderers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusIcon {
    Ok,
    Warning,
    Error,
    Attention,
}

impl StatusIcon {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warn",
            Self::Error => "error",
            Self::Attention => "attention",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StatusIcon;

    #[test]
    fn status_icon_labels_match_existing_status_words() {
        assert_eq!(StatusIcon::Ok.label(), "ok");
        assert_eq!(StatusIcon::Warning.label(), "warn");
        assert_eq!(StatusIcon::Error.label(), "error");
        assert_eq!(StatusIcon::Attention.label(), "attention");
    }
}
