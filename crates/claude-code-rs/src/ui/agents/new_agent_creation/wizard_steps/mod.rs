//! Individual create-agent wizard step renderers.

#[allow(dead_code)]
pub mod color_step;
#[allow(dead_code)]
pub mod confirm_step;
#[allow(dead_code)]
pub mod confirm_step_wrapper;
#[allow(dead_code)]
pub mod description_step;
#[allow(dead_code)]
pub mod generate_step;
#[allow(dead_code)]
pub mod location_step;
#[allow(dead_code)]
pub mod memory_step;
#[allow(dead_code)]
pub mod method_step;
#[allow(dead_code)]
pub mod model_step;
#[allow(dead_code)]
pub mod prompt_step;
#[allow(dead_code)]
pub mod tools_step;
#[allow(dead_code)]
pub mod type_step;

pub fn render_step_frame(title: &str, body: impl AsRef<str>, complete: bool) -> String {
    format!(
        "{}\nstatus: {}\n{}",
        title,
        if complete { "complete" } else { "needs input" },
        body.as_ref()
    )
}
