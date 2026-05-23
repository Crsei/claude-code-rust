pub(crate) const SCRIPTS_LOG_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/logs/pty_tui_e2e_scripts"
);

mod test1_login_structure;
mod test2_full_access;
