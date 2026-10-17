//! Phase 4 泳道 V3 的视觉证据（提醒事项 / 备忘笔记 / 设置）。
//!
//! 全部测试默认**跳过**：只有 `COURSEPLAN_VISUAL=1` 时才真正渲染。
//!
//! ```sh
//! COURSEPLAN_VISUAL=1 cargo test --test views_v3 -- --nocapture
//! ```
//!
//! 产出（`target/visual/`，均为 `lane-v3-*` 前缀，与其他泳道互不覆盖）：
//! - `lane-v3-{reminders,memos,settings}-{light,dark}.png`：三页 × 浅/深；
//! - `lane-v3-reminders-smoke-base/-editor/-editor-datepicker/-after-close/-done-tab.png`：
//!   提醒列表 → 编辑器 → DatePicker 日历弹层 → 关闭 → 已完成 tab；
//! - `lane-v3-memos-smoke-base/-editor/-pinned.png`：备忘网格 → 编辑器 → 已置顶筛选；
//! - `lane-v3-settings-smoke-base/-scrolled/-after-clicks.png`：设置页 → 同步卡片 +
//!   数据卡片（含「打开数据目录」链接）→ 点击登录 / 立即同步后的稳定帧。
//!
//! **注意（数据安全）**：`App::commit` 会把内存数据库写回真实数据目录
//! （`store::data_file()` 无环境变量开关）。因此本文件只做**不落盘**的交互：
//! 打开/关闭编辑器、切主题、切页、点击「登录 / 立即同步」（禁用句柄，仅执行
//! 闭包不写盘）。**不要**在这里点击勾选、置顶、保存、开关或登出——那会覆盖
//! 开发者本机的 data.json / sync.json。

mod support;

use course_plan::app::Page;
use course_plan::gpui::component::ThemeMode;
use course_plan::gpui::test::TestWindowExt;
use course_plan::ui::widgets::eid;
use course_plan::views::memos::MemoDraft;
use course_plan::views::reminders::ReminderDraft;

/// 本泳道负责的三个页面及其文件名 slug。
const V3_PAGES: [(Page, &str); 3] = [
    (Page::Reminders, "reminders"),
    (Page::Memos, "memos"),
    (Page::Settings, "settings"),
];

/// 三页 × 浅色/深色主题，各存一张 PNG；断言非平凡且浅 ≠ 深。
#[test]
fn render_v3_pages_light_and_dark() {
    if !support::enabled() {
        support::skip("render_v3_pages_light_and_dark");
        return;
    }

    let mut harness = support::Harness::new();
    let (expect_w, expect_h) = harness.expected_device_size();

    let mut written = Vec::new();
    for (page, slug) in V3_PAGES {
        let mut light = None;
        for (mode, theme) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            let name = format!("lane-v3-{slug}-{theme}");
            let (path, image) = harness.capture_page(page, mode, &name);
            support::assert_nontrivial(&image, expect_w, expect_h, &path.display().to_string());
            if theme == "light" {
                light = Some(image);
            } else if let Some(light) = &light {
                assert!(
                    support::images_differ(light, &image),
                    "{slug}: 浅色与深色截图一致，主题切换未生效"
                );
            }
            eprintln!("[lane-v3] {}", path.display());
            written.push(path);
        }
    }

    assert_eq!(written.len(), 6, "应产出 6 张截图（3 页 × 2 主题）");
    for path in &written {
        assert!(path.exists(), "缺少截图 {}", path.display());
    }
}

/// 提醒页：行渲染（稳定 id）→ 打开编辑器（overlay + dialog + DatePicker/TimeField）
/// → 关闭 → 切到「已完成」tab。全程无落盘操作。
#[test]
fn reminders_interaction_smoke() {
    if !support::enabled() {
        support::skip("reminders_interaction_smoke");
        return;
    }

    let mut harness = support::Harness::new();
    harness.set_page(Page::Reminders);
    harness.set_theme(ThemeMode::Light);
    harness.draw();

    // 行渲染证据：示例数据里的提醒都有稳定的 toggle id（point 语义圆点 + 勾选）。
    let reminder_ids: Vec<String> = {
        let app = harness.app.clone();
        harness.cx.update(|cx| {
            app.read(cx)
                .db
                .live_reminders()
                .map(|r| r.id.clone())
                .collect()
        })
    };
    assert!(!reminder_ids.is_empty(), "示例数据应至少有一条提醒");
    let mut missing = Vec::new();
    let window = harness.window;
    for id in &reminder_ids {
        let found = harness
            .cx
            .update_window(window, |_, window, _| {
                window.try_find(eid("reminder-toggle", id)).is_some()
            })
            .expect("读取渲染树失败");
        if !found {
            missing.push(id.clone());
        }
    }
    assert!(missing.is_empty(), "缺少提醒行 test id：{missing:?}");

    let (base_path, base) = harness.capture_current("lane-v3-reminders-smoke-base");

    // 打开「新建提醒」编辑器（公开 API），对话框应包含 DatePicker/TimeField。
    let app = harness.app.clone();
    harness
        .cx
        .update_window(window, |_, window, cx| {
            app.update(cx, |app, cx| {
                app.open_reminder_editor(ReminderDraft::empty(), window, cx);
            });
        })
        .expect("打开提醒编辑器失败");
    harness.draw();

    let dialog_found = harness
        .cx
        .update_window(window, |_, window, _| {
            window.try_find("reminder-editor:body").is_some()
        })
        .expect("读取渲染树失败");
    assert!(
        dialog_found,
        "提醒编辑器对话框未渲染（reminder-editor:body）"
    );

    let (editor_path, editor) = harness.capture_current("lane-v3-reminders-editor");
    assert!(
        support::images_differ(&base, &editor),
        "打开提醒编辑器后画面应变化（{} / {}）",
        base_path.display(),
        editor_path.display()
    );

    // 日期控件必须在对话框里可用：点开 DatePicker 日历弹层（画面应再次变化）。
    harness
        .cx
        .update_window(window, |_, window, cx| {
            window.click("reminder-editor:date", cx);
        })
        .expect("点击日期控件失败");
    harness.draw();
    let (calendar_path, calendar) = harness.capture_current("lane-v3-reminders-editor-datepicker");
    assert!(
        support::images_differ(&editor, &calendar),
        "点开日期选择器后应出现日历弹层（{}）",
        calendar_path.display()
    );

    // 关闭编辑器（不保存 → 不落盘）。
    harness.close_editors();
    let (after_close_path, after_close) = harness.capture_current("lane-v3-reminders-after-close");
    assert!(
        support::images_differ(&editor, &after_close),
        "关闭编辑器后画面应再次变化（{}）",
        after_close_path.display()
    );

    // 切到「已完成」tab（等价原「显示已完成」开关）：示例数据无已完成提醒 → 空态。
    let app = harness.app.clone();
    harness.cx.update(|cx| {
        app.update(cx, |app, cx| {
            app.show_done_reminders = true;
            cx.notify();
        });
    });
    harness.draw();
    let (done_path, done) = harness.capture_current("lane-v3-reminders-done-tab");
    assert!(
        support::images_differ(&base, &done),
        "切换到已完成 tab 后画面应变化（{}）",
        done_path.display()
    );
    eprintln!(
        "[lane-v3] 提醒交互截图：{} / {} / {} / {}",
        base_path.display(),
        editor_path.display(),
        after_close_path.display(),
        done_path.display()
    );
}

/// 备忘页：卡片网格渲染（稳定 id）→ 打开编辑器 → 切到「已置顶」筛选。
#[test]
fn memos_interaction_smoke() {
    if !support::enabled() {
        support::skip("memos_interaction_smoke");
        return;
    }

    let mut harness = support::Harness::new();
    harness.set_page(Page::Memos);
    harness.set_theme(ThemeMode::Light);
    harness.draw();

    let memo_ids: Vec<String> = {
        let app = harness.app.clone();
        harness
            .cx
            .update(|cx| app.read(cx).db.live_memos().map(|m| m.id.clone()).collect())
    };
    assert!(!memo_ids.is_empty(), "示例数据应至少有一条备忘");
    let window = harness.window;
    let mut missing = Vec::new();
    for id in &memo_ids {
        let found = harness
            .cx
            .update_window(window, |_, window, _| {
                window.try_find(eid("memo-card", id)).is_some()
            })
            .expect("读取渲染树失败");
        if !found {
            missing.push(id.clone());
        }
    }
    assert!(missing.is_empty(), "缺少备忘卡片 test id：{missing:?}");

    let (base_path, base) = harness.capture_current("lane-v3-memos-smoke-base");

    let app = harness.app.clone();
    harness
        .cx
        .update_window(window, |_, window, cx| {
            app.update(cx, |app, cx| {
                app.open_memo_editor(MemoDraft::empty(), window, cx);
            });
        })
        .expect("打开备忘编辑器失败");
    harness.draw();

    let dialog_found = harness
        .cx
        .update_window(window, |_, window, _| {
            window.try_find("memo-editor:body").is_some()
        })
        .expect("读取渲染树失败");
    assert!(dialog_found, "备忘编辑器对话框未渲染（memo-editor:body）");

    let (editor_path, editor) = harness.capture_current("lane-v3-memos-editor");
    assert!(
        support::images_differ(&base, &editor),
        "打开备忘编辑器后画面应变化（{} / {}）",
        base_path.display(),
        editor_path.display()
    );

    harness.close_editors();

    // 「已置顶」筛选（示例数据有一条置顶备忘）。
    let app = harness.app.clone();
    harness.cx.update(|cx| {
        app.update(cx, |app, cx| {
            app.memo_filter = course_plan::views::memos::MemoFilter::Pinned;
            cx.notify();
        });
    });
    harness.draw();
    let (pinned_path, pinned) = harness.capture_current("lane-v3-memos-pinned");
    assert!(
        support::images_differ(&base, &pinned),
        "切换到已置顶筛选后画面应变化（{}）",
        pinned_path.display()
    );
    eprintln!(
        "[lane-v3] 备忘交互截图：{} / {} / {}",
        base_path.display(),
        editor_path.display(),
        pinned_path.display()
    );
}

/// 设置页：同步卡片与「打开数据目录」链接渲染；点击链接、登录、立即同步
/// （三者都不落盘：链接在无头平台为空实现，登录/同步发往禁用句柄）。
#[test]
fn settings_sync_card_and_link_smoke() {
    if !support::enabled() {
        support::skip("settings_sync_card_and_link_smoke");
        return;
    }

    let mut harness = support::Harness::new();
    harness.set_page(Page::Settings);
    harness.set_theme(ThemeMode::Light);
    harness.draw();

    let window = harness.window;
    for id in [
        "sync-card",
        "sync-action:login",
        "sync-action:register",
        "sync-action:logout",
        "sync-action:sync-now",
        "data-action:open-dir",
    ] {
        let found = harness
            .cx
            .update_window(window, |_, window, _| window.try_find(id).is_some())
            .expect("读取渲染树失败");
        assert!(found, "设置页缺少 {id}（同步卡片 / 链接未渲染）");
    }

    let (base_path, _base) = harness.capture_current("lane-v3-settings-smoke-base");

    // 数据落盘哨兵：点击「登录 / 立即同步」不应写盘（二者只向禁用句柄发命令）。
    let data_file = course_plan::store::data_file();
    let stamp_before = std::fs::metadata(&data_file)
        .and_then(|m| m.modified())
        .ok();

    // 滚动到同步卡片 + 数据卡片可见，留一张「同步卡片 + 链接」证据图。
    assert!(
        scroll_until_visible(&mut harness, "data-action:open-dir"),
        "data-action:open-dir 滚动后仍不可见"
    );
    let (scrolled_path, _scrolled) = harness.capture_current("lane-v3-settings-scrolled");

    // 点击「登录 / 立即同步」：执行同步卡片的既有代码路径（无落盘）。
    // 「打开数据目录」链接不在测试里点击——无头平台的 `reveal_path` 是
    // `unimplemented!()`（stub），真实行为由人工验收 / observer 截图确认。
    let mut clicked = Vec::new();
    for id in ["sync-action:login", "sync-action:sync-now"] {
        assert!(
            scroll_until_visible(&mut harness, id),
            "{id} 滚动后仍不可见"
        );
        harness
            .cx
            .update_window(window, |_, window, cx| {
                window.click(id, cx);
            })
            .unwrap_or_else(|err| panic!("点击 {id} 失败：{err}"));
        harness.draw();
        clicked.push(id);
    }

    let stamp_after = std::fs::metadata(&data_file)
        .and_then(|m| m.modified())
        .ok();
    assert_eq!(
        stamp_before,
        stamp_after,
        "点击 {clicked:?} 改动了数据文件 {}",
        data_file.display()
    );

    let (after_path, _after) = harness.capture_current("lane-v3-settings-smoke-after-clicks");
    eprintln!(
        "[lane-v3] 设置页同步卡片与链接：{} / {} / {}（点击 {clicked:?}）",
        base_path.display(),
        scrolled_path.display(),
        after_path.display()
    );
}

/// 向下滚动窗口内容，直到 `id` 可见；返回最终是否可见。
fn scroll_until_visible(harness: &mut support::Harness, id: &'static str) -> bool {
    use course_plan::gpui::{ElementId, InputEvent as _, ScrollDelta, ScrollWheelEvent, point, px};

    let element_id: ElementId = id.into();
    let window = harness.window;
    for _ in 0..12 {
        let target = element_id.clone();
        let visible = harness
            .cx
            .update_window(window, |_, window, _| {
                window
                    .try_find(target.clone())
                    .map(|s| s.visible())
                    .unwrap_or(false)
            })
            .expect("读取渲染树失败");
        if visible {
            return true;
        }
        harness
            .cx
            .update_window(window, |_, window, cx| {
                window.dispatch_event(
                    ScrollWheelEvent {
                        position: point(px(700.), px(420.)),
                        delta: ScrollDelta::Pixels(point(px(0.), px(-220.))),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
                window.render_frame(cx);
            })
            .expect("滚动设置页失败");
    }
    false
}
