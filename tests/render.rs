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
use course_plan::gpui::component::ThemeMode;

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
