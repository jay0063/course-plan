//! 无头渲染 / 截图测试（Phase 1 · 泳道 V）。
//!
//! 全部测试默认**跳过**：只有 `COURSEPLAN_VISUAL=1` 时才真正渲染（保证无 GPU 环境
//! 下普通 `cargo test` 依旧绿色）。启用方式：
//!
//! ```sh
//! COURSEPLAN_VISUAL=1 cargo test --test render -- --nocapture
//! ```
//!
//! 无 GPU 机器需启用 lavapipe（软件 Vulkan）：
//!
//! ```sh
//! VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
//!   COURSEPLAN_VISUAL=1 cargo test --test render -- --nocapture
//! ```
//!
//! 产出：`target/visual/*.png`（8 页 × 浅/深 = 16 张，外加交互冒烟截图）。

mod support;

use course_plan::app::Page;
use course_plan::gpui::ElementId;
use course_plan::gpui::component::ThemeMode;
use course_plan::gpui::test::TestWindowExt;

/// 8 个页面及其文件名 slug。
const PAGES: [(Page, &str); 8] = [
    (Page::Dashboard, "dashboard"),
    (Page::Courses, "courses"),
    (Page::Schedule, "schedule"),
    (Page::Events, "events"),
    (Page::Reminders, "reminders"),
    (Page::CheckIns, "checkins"),
    (Page::Memos, "memos"),
    (Page::Settings, "settings"),
];

/// 渲染全部 8 页 × 浅色/深色主题，各存一张 PNG，并断言截图非平凡。
#[test]
fn render_all_pages_light_and_dark() {
    if !support::enabled() {
        support::skip("render_all_pages_light_and_dark");
        return;
    }

    let mut harness = support::Harness::new();
    let (expect_w, expect_h) = harness.expected_device_size();

    let mut written = Vec::new();
    for (page, slug) in PAGES {
        let mut light = None;
        for (mode, theme) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            let name = format!("{slug}-{theme}");
            let (path, image) = harness.capture_page(page, mode, &name);
            support::assert_nontrivial(&image, expect_w, expect_h, &path.display().to_string());
            if theme == "light" {
                light = Some(image);
            } else if let Some(light) = &light {
                // 浅色与深色确实不同，佐证主题切换生效（而不是两张一样的图）。
                assert!(
                    support::images_differ(light, &image),
                    "{slug}: 浅色与深色截图一致，主题切换未生效"
                );
            }
            written.push(path);
        }
    }

    assert_eq!(written.len(), 16, "应产出 16 张截图（8 页 × 2 主题）");
    for path in &written {
        assert!(path.exists(), "缺少截图 {}", path.display());
    }

    eprintln!(
        "[render] 已写出 {} 张截图到 {}（{}×{}）",
        written.len(),
        support::output_dir().display(),
        expect_w,
        expect_h
    );
}

/// Shell 稳定 test id（§6.8）：必须在渲染树里可被找到。
///
/// 这是 Phase 4 三泳道 QA 的锚点——id 一旦漂移，此测试失败。
#[test]
fn shell_element_ids_are_stable() {
    if !support::enabled() {
        support::skip("shell_element_ids_are_stable");
        return;
    }

    let mut harness = support::Harness::new();
    harness.draw();
    let window = harness.window;
    let mut missing = Vec::new();
    for id in [
        "ui:brand:",
        "ui:topbar:theme-toggle",
        "ui:topbar:quick-add",
        "ui:topbar:sync-pill",
        "ui:sidebar:sync-indicator",
        "nav:今日概览",
    ] {
        let found = harness
            .cx
            .update_window(window, |_, window, _| window.try_find(id).is_some())
            .expect("读取渲染树失败");
        if !found {
            missing.push(id);
        }
    }
    assert!(missing.is_empty(), "缺少稳定 test id：{missing:?}");
}

/// 交互冒烟：导航到「课程管理」，经公开 API 打开课程编辑器、输入文本、关闭，
/// 全程不应 panic，且各阶段截图相对基线发生变化。
#[test]
fn interaction_course_editor_smoke() {
    if !support::enabled() {
        support::skip("interaction_course_editor_smoke");
        return;
    }

    let mut harness = support::Harness::new();

    // 基线：课程管理页（浅色）。
    let (_, base) = harness.capture_page(Page::Courses, ThemeMode::Light, "smoke-courses-base");

    // 经公开 API 打开「新建课程」编辑器。
    harness.open_course_editor();
    harness.draw();

    // 向当前焦点派发文本（best-effort，见 support::Harness::input 说明）。
    harness.input("Robotics");

    let (editor_path, editor) = harness.capture_current("smoke-course-editor");
    assert!(
        support::images_differ(&base, &editor),
        "打开课程编辑器后画面应变化（{}）",
        editor_path.display()
    );

    // 关闭编辑器：不应 panic。
    harness.close_editors();
    let (_, after_close) = harness.capture_current("smoke-courses-after-close");
    assert!(
        support::images_differ(&editor, &after_close),
        "关闭编辑器后画面应再次变化"
    );
}

/// 换页即清确认弹层（Gate 4 oracle 携带项）：删除确认挂在 GPUI `Global`、
/// 打卡归档确认挂在 thread-local——离页再回时确认框不得「阴魂不散」。
#[test]
fn interaction_confirm_dialogs_clear_on_page_change() {
    if !support::enabled() {
        support::skip("interaction_confirm_dialogs_clear_on_page_change");
        return;
    }

    let mut harness = support::Harness::new();
    let app = harness.app.clone();

    // MARK: 课程删除确认 → 切页 → 切回：确认框必须已被丢弃
    harness.set_page(Page::Courses);
    harness.set_theme(ThemeMode::Light);
    harness.draw();
    let course_id = harness
        .cx
        .update(|cx| app.read(cx).db.live_courses().next().map(|c| c.id.clone()))
        .expect("示例数据里至少应有一门课程");

    click(&mut harness, format!("course-del:{course_id}"));
    harness.draw();
    let ok_id = format!("delete-course:{course_id}:ok");
    assert!(
        in_tree(&mut harness, ok_id.clone()),
        "点删除后确认框应出现（{ok_id}）"
    );
    let (_, open) = harness.capture_current("smoke-confirm-courses-open");

    harness.set_page(Page::Dashboard);
    harness.draw();
    harness.set_page(Page::Courses);
    harness.draw();
    assert!(
        !in_tree(&mut harness, ok_id.clone()),
        "离页再回后删除确认框应已被丢弃（{ok_id}）"
    );
    let (return_path, returned) = harness.capture_current("smoke-confirm-courses-return");
    assert!(
        support::images_differ(&open, &returned),
        "回到课程页后确认遮罩应消失（{}）",
        return_path.display()
    );

    // MARK: 打卡归档确认 → 切页 → 切回：同样的清理
    harness.set_page(Page::CheckIns);
    harness.draw();
    let checkin_id = harness
        .cx
        .update(|cx| app.read(cx).db.live_checkins().next().map(|c| c.id.clone()))
        .expect("示例数据里至少应有一个打卡任务");

    click(&mut harness, format!("checkin-arch-all:{checkin_id}"));
    harness.draw();
    let arch_ok = format!("checkin-arch:{checkin_id}:ok");
    assert!(
        in_tree(&mut harness, arch_ok.clone()),
        "点归档后二次确认应出现（{arch_ok}）"
    );
    harness.capture_current("smoke-confirm-checkins-open");

    harness.set_page(Page::Events);
    harness.draw();
    harness.set_page(Page::CheckIns);
    harness.draw();
    assert!(
        !in_tree(&mut harness, arch_ok.clone()),
        "离页再回后归档确认框应已被丢弃（{arch_ok}）"
    );
    let (arch_path, _) = harness.capture_current("smoke-confirm-checkins-return");
    eprintln!(
        "[render] 换页清理确认弹层：{} / {}",
        return_path.display(),
        arch_path.display()
    );
}

/// 点击一个已渲染的元素（真实的命中测试 + 渲染一帧）。
fn click(harness: &mut support::Harness, id: impl Into<ElementId>) {
    let id = id.into();
    let window = harness.window;
    harness
        .cx
        .update_window(window, |_, window, cx| {
            window.click(id, cx);
        })
        .expect("点击失败");
}

/// 元素是否在当前渲染树里。
fn in_tree(harness: &mut support::Harness, id: impl Into<ElementId>) -> bool {
    let id = id.into();
    let window = harness.window;
    harness
        .cx
        .update_window(window, |_, window, _| window.try_find(id).is_some())
        .expect("读取渲染树失败")
}
