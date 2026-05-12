use super::*;

#[test]
fn test_all_commands_registered() {
    let cmds = get_all_commands();
    assert!(cmds.len() >= 20, "Expected at least 20 commands");
    let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"help"));
    assert!(names.contains(&"clear"));
    assert!(names.contains(&"config"));
    assert!(names.contains(&"diff"));
    assert!(names.contains(&"exit"));
    assert!(names.contains(&"version"));
    assert!(names.contains(&"model"));
    assert!(names.contains(&"cost"));
    assert!(names.contains(&"skills"));
    assert!(names.contains(&"mcp"));
    assert!(names.contains(&"plugin"));
    assert!(names.contains(&"experimental"));
    assert!(names.contains(&"coordinator"));
    assert!(names.contains(&"review"));
    assert!(names.contains(&"security-review"));
    assert!(names.contains(&"recap"));
    assert!(names.contains(&"branch"));
    assert!(names.contains(&"gbranch"));
    assert!(names.contains(&"hooks"));
    assert!(names.contains(&"agents"));
    assert!(names.contains(&"doctor"));
    assert!(names.contains(&"tasks"));
    assert!(names.contains(&"loop"));
    assert!(names.contains(&"schedule"));
    assert!(names.contains(&"team-onboarding"));
    assert!(names.contains(&"remote"));
}

#[test]
fn test_commands_sorted_with_init_first() {
    let cmds = get_all_commands();
    let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names.first().copied(), Some("init"));

    let rest = &names[1..];
    let mut sorted = rest.to_vec();
    sorted.sort();
    assert_eq!(rest, sorted.as_slice());
}

#[test]
fn test_find_command_by_name() {
    assert!(find_command("help").is_some());
    assert!(find_command("clear").is_some());
    assert!(find_command("nonexistent").is_none());
}

#[test]
fn test_find_command_by_alias() {
    assert!(find_command("h").is_some());
    assert!(find_command("?").is_some());
    assert!(find_command("settings").is_some());
    assert!(find_command("quit").is_some());
    assert!(find_command("q").is_some());
    assert!(find_command("v").is_some());
    assert!(find_command("usage").is_some());
    assert!(find_command("ctx").is_some());
    assert!(find_command("coord").is_some());
    assert!(find_command("perms").is_some());
    assert!(find_command("br").is_some());
    assert!(find_command("gitbranch").is_some());
    assert!(find_command("exp").is_some());
    assert!(find_command("experiments").is_some());
    assert!(find_command("mem").is_some());
    assert!(
        find_command("agent").is_none(),
        "/agent is intentionally not an alias; /agents is the canonical list/detail command"
    );
    assert!(find_command("cron").is_some());
    assert!(find_command("teamonboarding").is_some());
}

#[test]
fn test_parse_command_input() {
    assert!(parse_command_input("/help").is_some());
    assert!(parse_command_input("/config set model SOTA").is_some());
    assert!(parse_command_input("not a command").is_none());
    assert!(parse_command_input("").is_none());
}
