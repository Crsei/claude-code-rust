use serde_json::Value;

use crate::ui::hooks::hooks_config_menu::HookConfigSummary;
use crate::ui::hooks::select_event_mode::{HOOK_EVENTS, HookEvent};
pub(crate) fn hook_summary(event: HookEvent, value: Option<&Value>) -> HookConfigSummary {
    let Some(configs) = value.and_then(Value::as_array) else {
        return HookConfigSummary {
            event,
            matcher_count: 0,
            command_count: 0,
        };
    };

    let command_count = configs
        .iter()
        .filter_map(|config| config.get("hooks"))
        .filter_map(Value::as_array)
        .map(Vec::len)
        .sum();
    HookConfigSummary {
        event,
        matcher_count: configs.len(),
        command_count,
    }
}

pub(crate) fn hook_event_order(event: HookEvent) -> usize {
    HOOK_EVENTS
        .iter()
        .position(|candidate| *candidate == event)
        .unwrap_or(usize::MAX)
}
