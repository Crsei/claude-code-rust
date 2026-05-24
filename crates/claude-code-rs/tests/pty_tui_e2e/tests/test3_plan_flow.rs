use crate::script::{TestCase, TestKey, TestRunner, TestStep};
use crate::tests::SCRIPTS_LOG_ROOT;
use std::time::Duration;

/// 测试：/plan 进入 plan mode，强制询问用户问题（AskUserQuestion），退出 plan mode。
///
/// 关键点：提示词要求 Claude 在创建计划前必须先问 3 个澄清问题，
/// 强制触发 AskUserQuestion 流程，测试 Up/Down 导航和 Enter 提交。
///
/// 流程：启动 TUI（bypass 权限模式）→ /plan（带强制性提问指令）→
///       等待 AskUserQuestion 对话框 → 导航选择 → 提交 → 等待 ExitPlanMode →
///       检查无 panic
#[test]
#[ignore = "requires real API key"]
fn script_plan_flow() {
    let case = TestCase::new("plan_flow")
        .log_root(SCRIPTS_LOG_ROOT)
        .permission_mode("bypass")
        .timeout(Duration::from_secs(300))
        .step(TestStep::SkipTrustGate)
        .step(TestStep::Wait(Duration::from_secs(2)))
        .step(TestStep::Snapshot("initial".into()))
        .step(TestStep::Command(
            "plan I want to add a new feature to the project. \
             There are multiple possible approaches and I'm not sure which is best. \
             Before you start planning, you MUST ask me questions first \
             using AskUserQuestion to clarify: \
             1) what kind of feature, 2) where to put it, 3) how to test it. \
             Do NOT create the plan until I answer your questions. \
             Just explore the codebase first, then ask me."
                .into(),
        ))
        // 等待 plan mode 相关工具出现
        .step(TestStep::WaitForAny(
            vec![
                "Enter plan mode".into(),
                "plan mode".into(),
                "Read".into(),
                "Glob".into(),
                "Grep".into(),
            ],
            Duration::from_secs(120),
        ))
        .step(TestStep::Snapshot("plan_entered".into()))
        // 等待 AskUserQuestion 对话框出现（主要测试目标）
        .step(TestStep::WaitForAny(
            vec![
                "Need input".into(),
                "Choices".into(),
                "Up/Down choice".into(),
                "Exit plan mode".into(),
                "ExitPlanMode".into(),
                "plan.md".into(),
            ],
            Duration::from_secs(180),
        ))
        .step(TestStep::Snapshot("ask_question_or_plan".into()))
        // 在 AskUserQuestion 中导航选择：Down 选中选项，Enter 提交
        .step(TestStep::Wait(Duration::from_secs(2)))
        .step(TestStep::Key(TestKey::Down))
        .step(TestStep::Wait(Duration::from_millis(300)))
        .step(TestStep::Key(TestKey::Down))
        .step(TestStep::Wait(Duration::from_millis(300)))
        .step(TestStep::Key(TestKey::Enter))
        .step(TestStep::Wait(Duration::from_millis(500)))
        .step(TestStep::Key(TestKey::Enter))
        .step(TestStep::Wait(Duration::from_secs(3)))
        .step(TestStep::Snapshot("after_answer".into()))
        // 等待 plan mode 退出
        .step(TestStep::WaitForAny(
            vec![
                "Exit plan mode".into(),
                "ExitPlanMode".into(),
                "Exited plan".into(),
                "approved".into(),
            ],
            Duration::from_secs(180),
        ))
        .step(TestStep::Snapshot("plan_exited".into()))
        .step(TestStep::Wait(Duration::from_secs(2)))
        .step(TestStep::AssertNoPanic)
        .step(TestStep::Key(TestKey::CtrlC))
        .step(TestStep::Wait(Duration::from_millis(500)))
        .step(TestStep::Key(TestKey::CtrlC));

    TestRunner::new().run(&case).assert_no_errors();
}
