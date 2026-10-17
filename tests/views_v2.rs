//! Phase 4 · 泳道 V2（课程管理 / 计划表 / 日程安排）的视觉证据。
//!
//! 全部测试默认**跳过**：只有 `COURSEPLAN_VISUAL=1` 时才真正渲染（无 GPU 机器上
//! 普通 `cargo test` 保持绿色）。启用方式：
//!
//! ```sh
//! COURSEPLAN_VISUAL=1 cargo test --test views_v2 -- --nocapture
//! ```
//!
//! 产出：`target/visual/lane-v2-*.png`（本泳道三个页面 × 浅/深 + 有数据的日程页 +
//! 三个编辑器对话框）。只读取共享脚手架 `tests/support/mod.rs`，不修改它。

mod support;

use course_plan::app::Page;
use course_plan::gpui::component::ThemeMode;
use course_plan::gpui::test::TestWindowExt;
use course_plan::gpui::{AnyWindowHandle, Entity};
use course_plan::model::{self, EventItem};
use course_plan::ui::tokens::{SIDEBAR_W, SPACE_XL};
use course_plan::views::events::EventDraft;
use course_plan::views::schedule::SessionDraft;
use image::RgbaImage;

/// §4.3 契约：7 列最小宽 = 56 + 7×88 + 6×4。
const GRID_MIN_W: f32 = 696.;
/// §4.3 契约：右侧概况 + 安排清单栏宽。
const RAIL_W: f32 = 260.;
/// §4.3 契约：左右两栏间距（SPACE_LG = 16）。
const RAIL_GAP: f32 = 16.;

/// 本泳道三个页面。
const PAGES: [(Page, &str); 3] = [
    (Page::Courses, "courses"),
    (Page::Schedule, "schedule"),
    (Page::Events, "events"),
];

/// 渲染三个页面 × 浅/深主题，各存一张 `lane-v2-*.png`。
#[test]
fn lane_v2_pages_light_and_dark() {
    if !support::enabled() {
        support::skip("lane_v2_pages_light_and_dark");
        return;
    }

    let mut harness = support::Harness::new();
    let (expect_w, expect_h) = harness.expected_device_size();

    let mut written = Vec::new();
    for (page, slug) in PAGES {
        let mut light = None;
        for (mode, theme) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            let name = format!("lane-v2-{slug}-{theme}");
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

    assert_eq!(written.len(), 6, "应产出 6 张截图（3 页 × 2 主题）");
    for path in &written {
        assert!(path.exists(), "缺少截图 {}", path.display());
    }
    eprintln!(
        "[views_v2] 已写出 {} 张截图到 {}（{}×{}）",
        written.len(),
        support::output_dir().display(),
        expect_w,
        expect_h
    );
}

/// 计划表：1280 宽下周日列必须完整可见（§4.3 / §8）。
///
/// 两层证据：
/// 1. 宽度算式——内容区 = 窗口 1280 − 侧栏 256 − 页面左右 padding 40 = 984，
///    左栏 = 984 − 260（右栏）− 16（间距）= 708 ≥ 696（网格最小宽）；
/// 2. 像素证据——周日列唯一的课程「奥数」用 palette[6] 嫩芽 `#A6C84B` 色条，
///    必须能在「周日列应有的 x 区间」内找到该色像素。
#[test]
fn lane_v2_schedule_sunday_stays_visible() {
    if !support::enabled() {
        support::skip("lane_v2_schedule_sunday_stays_visible");
        return;
    }

    // 1. 布局算式（token 化，防止 padding / 栏宽漂移后静默裁切）。
    let content_w = support::WINDOW_W - f32::from(SIDEBAR_W) - 2.0 * f32::from(SPACE_XL);
    let left_col_w = content_w - RAIL_W - RAIL_GAP;
    assert!(
        left_col_w >= GRID_MIN_W,
        "左栏 {left_col_w} < 网格最小宽 {GRID_MIN_W}：周日列会被裁切"
    );

    // 2. 像素证据。
    let mut harness = support::Harness::new();
    let (_, image) =
        harness.capture_page(Page::Schedule, ThemeMode::Light, "lane-v2-schedule-light");

    let scale = support::WINDOW_W;
    let device_scale = image.width() as f32 / scale;
    // 周日列（第 7 列）的逻辑 x 区间：内容左缘 256+20，时间槽列 56，每列 88+4。
    let day_col = 88.0 + 4.0;
    let sunday_x0 = f32::from(SIDEBAR_W) + f32::from(SPACE_XL) + 56.0 + 6.0 * day_col;
    let sunday_x1 = sunday_x0 + 88.0;
    // 校验收敛：周日列必须落在左栏内（不被右栏覆盖）。
    assert!(
        sunday_x1 + f32::from(SPACE_XL) <= support::WINDOW_W - RAIL_W,
        "周日列右缘 {sunday_x1} 已进入右栏/窗口外：会被裁切"
    );

    let x_start = (sunday_x0 * device_scale) as u32;
    let x_end = ((sunday_x1 * device_scale) as u32).min(image.width().saturating_sub(1));
    let strip = find_color_in_band(&image, (0xA6, 0xC8, 0x4B), 12, x_start, x_end);
    assert!(
        strip.is_some(),
        "周日列 x[{x_start}..{x_end}]（设备像素）内找不到课程色条 #A6C84B \
         —— 周日列未渲染或被裁切"
    );
    eprintln!(
        "[views_v2] 周日列可见：左栏 {left_col_w} ≥ {GRID_MIN_W}，色条命中 {strip:?}（设备像素）"
    );
}

/// 日程页：注入两条日程（一条已完成）后渲染，验证列表 + 完成态视觉。
#[test]
fn lane_v2_events_day_list_with_events() {
    if !support::enabled() {
        support::skip("lane_v2_events_day_list_with_events");
        return;
    }

    let mut db = course_plan::store::sample_data();
    let math_id = db
        .live_courses()
        .find(|c| c.name == "数学")
        .map(|c| c.id.clone());
    let today = model::fmt_date(model::today());

    let mut meeting = EventItem::new("家长会", today.clone());
    meeting.start = "15:00".into();
    meeting.end = "16:00".into();
    meeting.location = "阶梯教室".into();
    meeting.course_id = math_id;
    meeting.note = "带成绩单".into();

    let mut rehearsal = EventItem::new("钢琴汇演彩排", today);
    rehearsal.start = "19:00".into();
    rehearsal.end = "20:30".into();
    rehearsal.location = "琴房 3F".into();
    rehearsal.done = true;

    db.events = vec![meeting, rehearsal];

    let mut harness = support::Harness::with_database(db);
    let (expect_w, expect_h) = harness.expected_device_size();

    let (empty_path, empty) =
        harness.capture_page(Page::Events, ThemeMode::Light, "lane-v2-events-light");
    // 上面这张是「有数据」的日程页；空态由 lane_v2_pages_light_and_dark 覆盖。
    support::assert_nontrivial(
        &empty,
        expect_w,
        expect_h,
        &empty_path.display().to_string(),
    );

    let (path, image) =
        harness.capture_page(Page::Events, ThemeMode::Dark, "lane-v2-events-filled-dark");
    support::assert_nontrivial(&image, expect_w, expect_h, &path.display().to_string());
    assert!(
        support::images_differ(&empty, &image),
        "日程页（有数据）浅色与深色截图一致"
    );

    // 完成态：勾选圆为 success 实心；浅色 success = #3D8246。
    let check = find_color_in_band(&empty, (0x3D, 0x82, 0x46), 12, 0, empty.width());
    assert!(
        check.is_some(),
        "日程页看不到已完成的 success 勾选圆——完成态视觉缺失"
    );
    eprintln!(
        "[views_v2] 日程页（有数据）已写出 {} / {}",
        empty_path.display(),
        path.display()
    );
}

/// 三个编辑器对话框：`overlay` + `dialog` 内 gpui-kit 原生控件（DatePicker / TimeField）。
#[test]
fn lane_v2_editors_render() {
    if !support::enabled() {
        support::skip("lane_v2_editors_render");
        return;
    }

    let mut harness = support::Harness::new();
    let (expect_w, expect_h) = harness.expected_device_size();

    // 课程编辑器（8 色色板 + 课程类型下拉）。
    harness.set_page(Page::Courses);
    harness.set_theme(ThemeMode::Light);
    harness.open_course_editor();
    harness.draw();
    let (path, image) = harness.capture_current("lane-v2-editor-course");
    support::assert_nontrivial(&image, expect_w, expect_h, &path.display().to_string());
    harness.close_editors();

    // 排课编辑器（TimeField 开始/结束）。
    harness.set_page(Page::Schedule);
    open_session_editor(&mut harness);
    let (path, image) = harness.capture_current("lane-v2-editor-session");
    support::assert_nontrivial(&image, expect_w, expect_h, &path.display().to_string());
    harness.close_editors();

    // 日程编辑器（DatePicker + TimeField）。
    harness.set_page(Page::Events);
    open_event_editor(&mut harness);
    let (path, image) = harness.capture_current("lane-v2-editor-event");
    support::assert_nontrivial(&image, expect_w, expect_h, &path.display().to_string());
    harness.close_editors();

    eprintln!("[views_v2] 三个编辑器截图已写出（含 DatePicker / TimeField）");
}

/// 删除二次确认：`overlay` + `alert_dialog`，按钮 id 唯一（`{prefix}:ok` / `:cancel`）。
///
/// 只点「取消」（不点「删除」）——`App::commit` 会写真实数据文件，测试不能落地删除。
#[test]
fn lane_v2_delete_confirm_dialog() {
    if !support::enabled() {
        support::skip("lane_v2_delete_confirm_dialog");
        return;
    }

    let mut harness = support::Harness::new();
    let (expect_w, expect_h) = harness.expected_device_size();
    harness.set_page(Page::Courses);
    harness.set_theme(ThemeMode::Light);
    harness.draw();
    let (_, base) = harness.capture_current("lane-v2-courses-delete-base");

    let app = harness.app.clone();
    let window = harness.window;
    let course_id = harness
        .cx
        .update(|cx| app.read(cx).db.live_courses().next().map(|c| c.id.clone()))
        .expect("示例数据里至少应有一门课程");

    // 点删除图标 → 确认框出现。
    harness
        .cx
        .update_window(window, |_, window, cx| {
            window.click(format!("course-del:{course_id}"), cx);
        })
        .expect("点击删除图标失败");
    harness.draw();

    // 唯一 id 必须存在（§3.12：`{prefix}:ok` / `{prefix}:cancel`）。
    let ok_id = format!("delete-course:{course_id}:ok");
    let cancel_id = format!("delete-course:{course_id}:cancel");
    let ids_in_tree = harness
        .cx
        .update_window(window, |_, window, _| {
            (
                window.try_find(ok_id.clone()).is_some(),
                window.try_find(cancel_id.clone()).is_some(),
            )
        })
        .expect("读取渲染树失败");
    assert!(
        ids_in_tree.0 && ids_in_tree.1,
        "确认框按钮 id 缺失：{ok_id} / {cancel_id}（{ids_in_tree:?}）"
    );

    let (path, confirm) = harness.capture_current("lane-v2-confirm-delete-course");
    support::assert_nontrivial(&confirm, expect_w, expect_h, &path.display().to_string());
    assert!(
        support::images_differ(&base, &confirm),
        "点删除后画面应出现确认遮罩（{}）",
        path.display()
    );

    // 遮罩证据：内容区角落像素应被 foreground α0.32 压暗。
    let dimmed = confirm.get_pixel(1800, 1500).0;
    let plain = base.get_pixel(1800, 1500).0;
    let sum = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
    assert!(
        sum(&dimmed) + 30 < sum(&plain),
        "确认框遮罩未压暗背景：dimmed={dimmed:?} plain={plain:?}"
    );

    // 点取消 → 真正关闭（on_cancel 回调生效）。
    harness
        .cx
        .update_window(window, |_, window, cx| {
            window.click(cancel_id.clone(), cx);
        })
        .expect("点击取消失败");
    harness.draw();
    let (_, after_cancel) = harness.capture_current("lane-v2-confirm-delete-after-cancel");
    assert!(
        support::images_differ(&confirm, &after_cancel),
        "取消后确认框应消失"
    );
    let restored = after_cancel.get_pixel(1800, 1500).0;
    assert!(
        (sum(&restored) - sum(&plain)).abs() <= 12,
        "取消后内容区应恢复原亮度：restored={restored:?} plain={plain:?}"
    );
    eprintln!("[views_v2] 删除确认框（唯一 id + 真取消）截图已写出");
}

/// 已归档课程的状态 chip：同一份示例数据，归档前后渲染结果必须不同。
#[test]
fn lane_v2_archived_course_chip() {
    if !support::enabled() {
        support::skip("lane_v2_archived_course_chip");
        return;
    }

    let mut active_db = course_plan::store::sample_data();
    let mut archived_db = course_plan::store::sample_data();
    let gym_id = archived_db
        .live_courses()
        .find(|c| c.name == "体育")
        .map(|c| c.id.clone())
        .expect("示例数据应有「体育」");
    for course in &mut active_db.courses {
        course.active = true;
    }
    for course in &mut archived_db.courses {
        course.active = course.id != gym_id;
    }

    let mut active = support::Harness::with_database(active_db);
    let (expect_w, expect_h) = active.expected_device_size();
    let (_, plain) = active.capture_page(
        Page::Courses,
        ThemeMode::Light,
        "lane-v2-courses-all-active-light",
    );

    let mut archived = support::Harness::with_database(archived_db);
    let (path, image) = archived.capture_page(
        Page::Courses,
        ThemeMode::Light,
        "lane-v2-courses-archived-light",
    );
    support::assert_nontrivial(&image, expect_w, expect_h, &path.display().to_string());
    assert!(
        support::images_differ(&plain, &image),
        "归档前后课程页应不同（已归档 chip + 名称变灰）"
    );
    eprintln!("[views_v2] 已归档 chip 截图已写出 {}", path.display());
}

// MARK: - 辅助

/// 打开排课编辑器（走 `App::open_session_editor` 公开 API）。
fn open_session_editor(harness: &mut support::Harness) {
    let app: Entity<course_plan::app::App> = harness.app.clone();
    let window: AnyWindowHandle = harness.window;
    harness
        .cx
        .update_window(window, |_, window, cx| {
            app.update(cx, |app, cx| {
                app.open_session_editor(SessionDraft::empty(), window, cx);
            });
        })
        .expect("打开排课编辑器失败");
    harness.draw();
}

/// 打开日程编辑器（走 `App::open_event_editor` 公开 API）。
fn open_event_editor(harness: &mut support::Harness) {
    let app: Entity<course_plan::app::App> = harness.app.clone();
    let window: AnyWindowHandle = harness.window;
    harness
        .cx
        .update_window(window, |_, window, cx| {
            app.update(cx, |app, cx| {
                app.open_event_editor(EventDraft::empty(), window, cx);
            });
        })
        .expect("打开日程编辑器失败");
    harness.draw();
}

/// 在 `x ∈ [x0, x1)` 的竖向通道内寻找指定 RGB（±tolerance）。
fn find_color_in_band(
    image: &RgbaImage,
    rgb: (u8, u8, u8),
    tolerance: i32,
    x0: u32,
    x1: u32,
) -> Option<(u32, u32)> {
    let (w, h) = image.dimensions();
    for x in x0.min(w)..x1.min(w) {
        for y in 0..h {
            let p = image.get_pixel(x, y).0;
            if (p[0] as i32 - rgb.0 as i32).abs() <= tolerance
                && (p[1] as i32 - rgb.1 as i32).abs() <= tolerance
                && (p[2] as i32 - rgb.2 as i32).abs() <= tolerance
            {
                return Some((x, y));
            }
        }
    }
    None
}
