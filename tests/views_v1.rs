//! Phase 4 · V1 泳道（dashboard + checkins）的视觉 / 交互冒烟证据。
//!
//! 只渲染本泳道两个页面，默认**跳过**；启用方式：
//!
//! ```sh
//! COURSEPLAN_VISUAL=1 cargo test --test views_v1 -- --nocapture
//! ```
//!
//! 产出：`target/visual/lane-v1-*.png`（含浅/深色两套 + 编辑器 / 归档确认 /
//! 庆祝瞬间等交互帧）。无 GPU 机器需要 lavapipe（同 `tests/render.rs`）。

mod support;

use std::time::Duration;

use course_plan::app::Page;
use course_plan::gpui::ElementId;
use course_plan::gpui::component::ThemeMode;
use course_plan::gpui::test::TestWindowExt;
use course_plan::model::{self, Database};
use course_plan::ui::widgets::eid;
use course_plan::views::checkins::CheckInDraft;

/// 重定向数据目录到 `target/`（`App::commit` 会写盘；绝不能碰真实 data.json）。
///
/// 进程内只设置一次；`set_var` 在 Rust 2024 下是 unsafe（与其他线程读 env 有竞态，
/// 但本测试二进制里所有 Harness 都在本调用之后创建，且只指向 target/ 内目录）。
fn redirect_store_to_temp() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = support::output_dir().join("_v1-store");
        std::fs::create_dir_all(&dir).expect("创建测试用数据目录失败");
        unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
        eprintln!("[views_v1] 写盘已重定向到 {}", dir.display());
    });
}

/// 读取当前 App 的数据库快照。
fn db_snapshot(harness: &mut support::Harness) -> Database {
    let app = harness.app.clone();
    harness.cx.update(|cx| app.read(cx).db.clone())
}

/// 点击一个已渲染的元素（内部会走真实的命中测试 + 渲染一帧）。
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

/// 今天处于 active 的打卡任务 id（顺序与页面一致）。
fn today_checkin_ids(harness: &mut support::Harness) -> Vec<String> {
    let today = model::today();
    db_snapshot(harness)
        .checkins_on(today)
        .iter()
        .map(|c| c.id.clone())
        .collect()
}

/// 两个"灵魂页" × 浅/深色：写出 `lane-v1-*.png`，断言非平凡且浅深确实不同。
#[test]
fn soul_pages_render_light_and_dark() {
    if !support::enabled() {
        support::skip("soul_pages_render_light_and_dark");
        return;
    }
    redirect_store_to_temp();

    let mut harness = support::Harness::new();
    let (expect_w, expect_h) = harness.expected_device_size();
    let mut written = Vec::new();

    for (page, slug) in [(Page::Dashboard, "dashboard"), (Page::CheckIns, "checkins")] {
        let mut light = None;
        for (mode, theme) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            let name = format!("lane-v1-{slug}-{theme}");
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
            written.push(path);
        }
    }

    assert_eq!(written.len(), 4, "应产出 4 张页面截图（2 页 × 2 主题）");
    for path in &written {
        assert!(path.exists(), "缺少截图 {}", path.display());
    }
    eprintln!(
        "[views_v1] 已写出 {} 张页面截图（{}×{}）：
{}",
        written.len(),
        expect_w,
        expect_h,
        written
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// 打卡页交互链：编辑器 → 取消；归档 → 二次确认 → 取消 / 确认；勾选 → 庆祝。
#[test]
fn checkin_interactions_do_not_panic_and_are_visual() {
    if !support::enabled() {
        support::skip("checkin_interactions_do_not_panic_and_are_visual");
        return;
    }
    redirect_store_to_temp();

    let mut harness = support::Harness::new();
    harness.set_page(Page::CheckIns);
    harness.set_theme(ThemeMode::Light);
    harness.draw();

    // MARK: 编辑器遮罩（§3.12 dialog），取消必须真的关闭
    let app = harness.app.clone();
    let window = harness.window;
    harness
        .cx
        .update_window(window, |_, window, cx| {
            app.update(cx, |app, cx| {
                app.open_checkin_editor(CheckInDraft::empty(), window, cx);
            });
        })
        .expect("打开打卡编辑器失败");
    harness.draw();
    let (editor_path, editor_img) = harness.capture_current("lane-v1-checkins-editor");
    assert!(
        harness
            .cx
            .update(|cx| app.read(cx).pending_checkin.is_some()),
        "编辑器未打开"
    );
    click(&mut harness, "checkin-cancel");
    harness.draw();
    assert!(
        harness
            .cx
            .update(|cx| app.read(cx).pending_checkin.is_none()),
        "取消后编辑器应关闭"
    );
    let (closed_path, closed_img) = harness.capture_current("lane-v1-checkins-editor-cancelled");
    assert!(
        support::images_differ(&editor_img, &closed_img),
        "关闭编辑器后画面应变化（{} / {}）",
        editor_path.display(),
        closed_path.display()
    );

    // MARK: 归档二次确认（§3.12 alert_dialog）；先取消，再确认
    let ids = today_checkin_ids(&mut harness);
    let first = ids.first().cloned().expect("示例数据应有今日打卡任务");
    click(&mut harness, eid("checkin-arch-pending", &first));
    harness.draw();
    let (confirm_path, confirm_img) = harness.capture_current("lane-v1-checkins-archive-confirm");
    assert!(confirm_img.width() > 0, "{} 应写出", confirm_path.display());
    assert!(
        !db_snapshot(&mut harness)
            .checkins
            .iter()
            .find(|c| c.id == first)
            .map(|c| c.archived)
            .unwrap_or(false),
        "二次确认前不应归档"
    );

    click(&mut harness, format!("checkin-arch:{first}:cancel"));
    harness.draw();
    let cancelled = db_snapshot(&mut harness);
    assert!(
        !cancelled
            .checkins
            .iter()
            .find(|c| c.id == first)
            .map(|c| c.archived)
            .unwrap_or(false),
        "点「取消」后不应归档"
    );
    harness.capture_current("lane-v1-checkins-archive-cancelled");

    // MARK: 勾选全部今日任务 → §3.17 庆祝瞬间
    for id in &ids {
        click(&mut harness, eid("checkin-toggle-pending", id));
    }
    harness.draw();
    let (celebration_path, celebration_img) =
        harness.capture_current("lane-v1-checkins-celebration");
    support::assert_nontrivial(
        &celebration_img,
        celebration_img.width(),
        celebration_img.height(),
        &celebration_path.display().to_string(),
    );
    let done = db_snapshot(&mut harness);
    let today = model::today();
    let active_today: Vec<_> = done
        .checkins
        .iter()
        .filter(|c| c.active_on(today) && !c.archived)
        .collect();
    assert!(
        !active_today.is_empty() && active_today.iter().all(|c| c.done_on(today)),
        "全部勾选后今日任务应都是 done"
    );
    assert!(
        active_today.iter().any(|c| c.streak() >= 1),
        "刚完成的连续打卡任务 streak 应 ≥ 1"
    );

    // 庆祝瞬间 1200ms 后自动回淡（推进测试时钟，不该 panic）
    harness.cx.advance_clock(Duration::from_millis(1400));
    harness.cx.run_until_parked();
    harness.draw();

    // MARK: 归档确认 → 通过：卡片移入「已归档」，可恢复
    // 勾选后卡片已从「待打卡」移到「今日已完成」；用「全部任务」列（该任务始终在其中）
    // 的按钮，顺便验证 3 列各自的 test id 互不冲突。
    click(&mut harness, eid("checkin-arch-all", &first));
    harness.draw();
    click(&mut harness, format!("checkin-arch:{first}:ok"));
    harness.draw();
    let archived = db_snapshot(&mut harness);
    assert!(
        archived
            .checkins
            .iter()
            .find(|c| c.id == first)
            .map(|c| c.archived)
            .unwrap_or(false),
        "确认后应归档"
    );
    harness.capture_current("lane-v1-checkins-archived");
    click(&mut harness, eid("checkin-restore", &first));
    harness.draw();
    assert!(
        !db_snapshot(&mut harness)
            .checkins
            .iter()
            .find(|c| c.id == first)
            .map(|c| c.archived)
            .unwrap_or(false),
        "「恢复」后应取消归档"
    );
}

/// 今日概览的打卡行：点击整行翻转 done（含 §3.17 弹跳），不 panic。
#[test]
fn dashboard_checkin_row_toggles() {
    if !support::enabled() {
        support::skip("dashboard_checkin_row_toggles");
        return;
    }
    redirect_store_to_temp();

    let mut harness = support::Harness::new();
    harness.set_page(Page::Dashboard);
    harness.set_theme(ThemeMode::Light);
    harness.draw();

    let today = model::today();
    let first = today_checkin_ids(&mut harness)
        .first()
        .cloned()
        .expect("示例数据应有今日打卡任务");
    let before = db_snapshot(&mut harness)
        .checkins
        .iter()
        .find(|c| c.id == first)
        .map(|c| c.done_on(today))
        .unwrap_or(false);

    click(&mut harness, eid("dash-checkin", &first));
    harness.draw();
    let after = db_snapshot(&mut harness)
        .checkins
        .iter()
        .find(|c| c.id == first)
        .map(|c| c.done_on(today))
        .unwrap_or(false);
    assert_ne!(before, after, "点击打卡行应翻转 done");

    let (path, image) = harness.capture_current("lane-v1-dashboard-after-toggle");
    assert!(path.exists(), "缺少截图 {}", path.display());
    assert!(image.width() > 0);

    // 切回暗色再抓一张，佐证弹跳 / 光晕在两个主题下都渲染
    harness.set_theme(ThemeMode::Dark);
    harness.draw();
    harness.capture_current("lane-v1-dashboard-after-toggle-dark");

    // 滚到底部抓一张：快捷入口 + 本学期概况也在证据里（滚轮事件打在可见元素上，
    // 由框架路由给页面的滚动容器）。
    harness.set_theme(ThemeMode::Light);
    harness.draw();
    harness
        .cx
        .update_window(harness.window, |_, window, cx| {
            window.scroll(
                "dash-goto-reminders",
                course_plan::gpui::ScrollDelta::Pixels(course_plan::gpui::point(
                    course_plan::gpui::px(0.),
                    course_plan::gpui::px(-420.),
                )),
                cx,
            );
        })
        .expect("滚动失败");
    harness.draw();
    let (scrolled_path, scrolled_img) = harness.capture_current("lane-v1-dashboard-scrolled");
    assert!(
        scrolled_path.exists() && scrolled_img.width() > 0,
        "缺少截图 {}",
        scrolled_path.display()
    );
}
