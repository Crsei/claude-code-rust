//! Ask-user-question permission request rendering.

use super::question_navigation_bar::render_question_navigation_bar;
use super::question_view::render_question_view;
use super::use_multiple_choice_state::MultipleChoiceState;

pub fn render_ask_user_question_permission_request(
    prompt: &str,
    state: &MultipleChoiceState,
    current: usize,
    total: usize,
) -> String {
    format!(
        "Ask user question\n{}\n{}",
        render_question_view(prompt, state),
        render_question_navigation_bar(current, total, current > 1, !state.submitted.is_empty())
    )
}
