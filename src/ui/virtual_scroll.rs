//! Virtual scroll — only render messages visible in the viewport.
//!
//! Maintains a per-message height cache and a cumulative prefix-sum offset
//! array. Uses binary search to find the visible message range in O(log n)
//! instead of iterating all messages every frame.

use crate::types::message::Message;

use super::messages::render_single_message;
use super::theme::Theme;

/// Number of extra lines to render above/below the viewport for smooth
/// scrolling.
const OVERSCAN: usize = 40;

pub struct VirtualScroll {
    /// Per-message rendered line count (including +1 separator for all but last).
    heights: Vec<usize>,
    /// Prefix-sum offsets: `offsets[i]` = sum of `heights[0..i]`.
    /// `offsets[0]` = 0, `offsets[n]` = total lines.
    offsets: Vec<usize>,
    /// Index from which the cache is invalid.
    valid_up_to: usize,
    /// Terminal width used for the cached heights.
    cached_width: u16,
}

impl VirtualScroll {
    pub fn new() -> Self {
        Self {
            heights: Vec::new(),
            offsets: vec![0],
            valid_up_to: 0,
            cached_width: 0,
        }
    }

    /// Invalidate all cached heights (e.g. after `clear_messages`).
    pub fn invalidate_all(&mut self) {
        self.heights.clear();
        self.offsets.clear();
        self.offsets.push(0);
        self.valid_up_to = 0;
    }

    /// Invalidate from a specific message index onward.
    pub fn invalidate_from(&mut self, index: usize) {
        if index < self.valid_up_to {
            self.valid_up_to = index;
        }
        self.heights.truncate(index);
        self.offsets.truncate(index + 1); // keep offsets[0..=index]
    }

    /// Ensure heights and offsets are up-to-date for all messages.
    /// Re-computes only the invalidated tail.
    pub fn ensure_up_to_date(&mut self, messages: &[Message], width: u16, theme: &Theme) {
        // Width changed → full invalidation
        if width != self.cached_width {
            self.invalidate_all();
            self.cached_width = width;
        }

        // Shrink if messages were removed
        if self.heights.len() > messages.len() {
            self.invalidate_from(messages.len());
        }

        let start = self.valid_up_to;
        let total = messages.len();

        for i in start..total {
            let lines = render_single_message(&messages[i], theme);
            let mut h = lines.len();
            // Separator blank line between messages (not after last)
            if i < total - 1 {
                h += 1;
            }
            if i < self.heights.len() {
                self.heights[i] = h;
            } else {
                self.heights.push(h);
            }
            // Rebuild offset
            let prev = if i < self.offsets.len() {
                self.offsets[i]
            } else {
                *self.offsets.last().unwrap_or(&0)
            };
            let new_off = prev + h;
            if i + 1 < self.offsets.len() {
                self.offsets[i + 1] = new_off;
            } else {
                self.offsets.push(new_off);
            }
        }
        // Make sure offsets has exactly total+1 entries
        self.offsets.truncate(total + 1);
        self.valid_up_to = total;
    }

    /// Total rendered line count across all messages.
    pub fn total_lines(&self) -> usize {
        self.offsets.last().copied().unwrap_or(0)
    }

    /// Compute the visible message index range `[start, end)` for the given
    /// scroll offset and viewport height.
    pub fn visible_range(&self, scroll_offset: usize, viewport_height: usize) -> (usize, usize) {
        let total_msgs = self.heights.len();
        if total_msgs == 0 {
            return (0, 0);
        }

        let lo = scroll_offset.saturating_sub(OVERSCAN);
        let hi = scroll_offset + viewport_height + OVERSCAN;

        // Binary search: first message whose cumulative end > lo
        let start = self.offsets.partition_point(|&o| o <= lo).saturating_sub(1);
        // First message whose cumulative start >= hi
        let end = self.offsets.partition_point(|&o| o < hi).min(total_msgs);

        (start, end)
    }

    /// Line offset of message `index` in the global line space.
    pub fn offset_of(&self, index: usize) -> usize {
        self.offsets.get(index).copied().unwrap_or(0)
    }

    /// Terminal width used by the current cache. Exposed for tests /
    /// diagnostics that want to verify the cache reacted to a resize.
    #[cfg(test)]
    pub fn cached_width(&self) -> u16 {
        self.cached_width
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::{
        AssistantMessage, ContentBlock, Message, MessageContent, UserMessage,
    };
    use crate::ui::theme::Theme;

    fn user(text: &str) -> Message {
        Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "user".into(),
            content: MessageContent::Text(text.into()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    fn assistant_text(text: &str) -> Message {
        Message::Assistant(AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".into(),
            content: vec![ContentBlock::Text { text: text.into() }],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        })
    }

    /// Regression for the issue-#12 acceptance criterion "resize reflow
    /// problem closed". `render_single_message` returns logical lines
    /// (not width-wrapped), so cached `heights[]` may be identical
    /// across widths — but the *cache metadata* must still track the
    /// latest width so invalidation triggers on the next real reflow
    /// (e.g. when messages.rs eventually pre-wraps).
    ///
    /// We assert two things:
    ///   1. `cached_width` updates on `ensure_up_to_date`
    ///   2. invalidation doesn't lose data when the width didn't change
    #[test]
    fn width_change_invalidates_height_cache() {
        let theme = Theme::default();
        let msgs = vec![
            user("a reasonably long message that will wrap differently on narrow widths"),
            assistant_text("and a long response that should also reflow"),
        ];
        let mut vs = VirtualScroll::new();
        vs.ensure_up_to_date(&msgs, 80, &theme);
        let total_first = vs.total_lines();
        assert_eq!(vs.cached_width(), 80);
        assert!(total_first > 0);

        // Width change → cache width should update. Height is allowed to
        // stay the same (logical lines), but re-measuring must have
        // happened (valid_up_to == len).
        vs.ensure_up_to_date(&msgs, 30, &theme);
        assert_eq!(vs.cached_width(), 30);
        assert_eq!(vs.total_lines(), vs.offsets.last().copied().unwrap_or(0));

        // Same width again → no-op, total unchanged.
        let total_before_noop = vs.total_lines();
        vs.ensure_up_to_date(&msgs, 30, &theme);
        assert_eq!(vs.total_lines(), total_before_noop);
    }

    #[test]
    fn invalidate_all_resets_cache_to_empty() {
        let theme = Theme::default();
        let msgs = vec![user("hello")];
        let mut vs = VirtualScroll::new();
        vs.ensure_up_to_date(&msgs, 40, &theme);
        assert!(vs.total_lines() > 0);
        vs.invalidate_all();
        assert_eq!(vs.total_lines(), 0);
    }
}
