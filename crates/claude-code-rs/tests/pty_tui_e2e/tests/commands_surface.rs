//! 命令板 surface 面板截图测试 — 验证所有 CommandSurface 面板渲染。
//!
//! 每个测试通过命令面板打开交互面板，截图并断言关键文本。
//! 所有测试均为离线测试，不需要 API key。
//!
//! 命令执行流程：
//! 1. `Command(cmd)` 发送 "/cmd\r"，触发命令面板打开并输入命令名
//! 2. 面板对无参数的命令直接执行，对有参数的命令插入到 prompt
//! 3. `Key(Enter)` 从 prompt 提交命令（对已直接执行的无副作用）
//! 4. 等待面板渲染、截图、断言

use crate::script::{TestCase, TestKey, TestRunner, TestStep};
use crate::tests::SCRIPTS_LOG_ROOT;
use std::time::Duration;

/// 执行斜杠命令打开 surface 面板，截图并断言关键文本。
fn surface_test(name: &str, cmd: &str, assert_text: &str) -> TestCase {
    TestCase::new(name)
        .log_root(SCRIPTS_LOG_ROOT)
        .timeout(crate::harness::QUICK_TIMEOUT)
        .cols(180)
        .rows(60)
        .step(TestStep::SkipTrustGate)
        .step(TestStep::Wait(Duration::from_millis(500)))
        .step(TestStep::Command(cmd.into()))
        .step(TestStep::Wait(Duration::from_millis(500)))
        // 第 2 个 Enter：从 prompt 提交命令（对已直接执行的命令无副作用）
        .step(TestStep::Key(TestKey::Enter))
        .step(TestStep::Wait(Duration::from_secs(1)))
        .step(TestStep::Snapshot(format!("{}_surface", name)))
        .step(TestStep::AssertScreenContains(assert_text.into()))
        .step(TestStep::AssertNoPanic)
}

// ─────────────────────────────────────────────────────────────────────────────
// 单个命令 surface 面板测试
// ─────────────────────────────────────────────────────────────────────────────

/// `/agents` surface：代理列表与详情面板。
#[test]
fn surface_agents() {
    TestRunner::new()
        .run(&surface_test("surface_agents", "agents", "+ Agents "))
        .assert_no_errors();
}

/// `/config` surface：配置面板（状态/模型/主题/使用量等标签页）。
#[test]
fn surface_config() {
    TestRunner::new()
        .run(&surface_test("surface_config", "config", "+ Config "))
        .assert_no_errors();
}

/// `/diff` surface：差异查看面板。
/// 测试工作区可能非 git 仓库，面板会显示错误提示，但仍然证明 DiffSurface 已打开。
#[test]
fn surface_diff() {
    TestRunner::new()
        .run(&surface_test("surface_diff", "diff", "Diff"))
        .assert_no_errors();
}

/// `/effort` surface：思考/effort 选择器面板（ConfigSurface 的 thinking 标签页）。
/// 当前模型可能不支持 reasoning levels，但仍可断言 "Effort" 章节标题。
#[test]
fn surface_effort() {
    TestRunner::new()
        .run(&surface_test("surface_effort", "effort", "Effort"))
        .assert_no_errors();
}

/// `/hooks` surface：钩子浏览面板。
#[test]
fn surface_hooks() {
    TestRunner::new()
        .run(&surface_test("surface_hooks", "hooks", "+ Hooks "))
        .assert_no_errors();
}

/// `/login` surface：登录/认证面板。
#[test]
fn surface_login() {
    TestRunner::new()
        .run(&surface_test(
            "surface_login",
            "login",
            "+ Login / Claude Code ",
        ))
        .assert_no_errors();
}

/// `/mcp` surface：MCP 服务器管理面板。
#[test]
fn surface_mcp() {
    TestRunner::new()
        .run(&surface_test("surface_mcp", "mcp", "+ MCP "))
        .assert_no_errors();
}

/// `/memory` surface：记忆文件管理面板。
#[test]
fn surface_memory() {
    TestRunner::new()
        .run(&surface_test("surface_memory", "memory", "+ Memory "))
        .assert_no_errors();
}

/// `/model` surface：模型选择器面板。
#[test]
fn surface_model() {
    TestRunner::new()
        .run(&surface_test("surface_model", "model", "+ Model "))
        .assert_no_errors();
}

/// `/permissions` surface：权限配置面板。
#[test]
fn surface_permissions() {
    TestRunner::new()
        .run(&surface_test(
            "surface_permissions",
            "permissions",
            "+ Permissions ",
        ))
        .assert_no_errors();
}

/// `/plugin` surface：插件管理面板。
#[test]
fn surface_plugin() {
    TestRunner::new()
        .run(&surface_test("surface_plugin", "plugin", "+ Plugins "))
        .assert_no_errors();
}

/// `/remote` surface：远程控制网关面板。
#[test]
fn surface_remote() {
    TestRunner::new()
        .run(&surface_test(
            "surface_remote",
            "remote",
            "Remote control gateway",
        ))
        .assert_no_errors();
}

/// `/resume` surface：会话恢复面板。
#[test]
fn surface_resume() {
    TestRunner::new()
        .run(&surface_test("surface_resume", "resume", "Resume Sessions"))
        .assert_no_errors();
}

/// `/sandbox` surface：沙箱配置面板。
#[test]
fn surface_sandbox() {
    TestRunner::new()
        .run(&surface_test("surface_sandbox", "sandbox", "+ Sandbox "))
        .assert_no_errors();
}

/// `/skills` surface：技能列表面板。
#[test]
fn surface_skills() {
    TestRunner::new()
        .run(&surface_test("surface_skills", "skills", "+ Skills "))
        .assert_no_errors();
}

/// `/tasks` surface：后台任务管理面板。
#[test]
fn surface_tasks() {
    TestRunner::new()
        .run(&surface_test("surface_tasks", "tasks", "Background tasks"))
        .assert_no_errors();
}

/// `/team` surface：团队成员管理面板。
#[test]
fn surface_team() {
    TestRunner::new()
        .run(&surface_test("surface_team", "team", "+ Team "))
        .assert_no_errors();
}
