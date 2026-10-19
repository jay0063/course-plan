//! 排课流程回归（V2 泳道补充）：课程下拉可选、保存落地、零安排/零课程两种边界。
//!
//! 缺陷背景（v0.1.0 用户反馈「排课无法选择课程」）：
//! 计划表页在**零安排**时走提前返回分支，根节点用了 `flex_1().min_h_0()`，
//! 而父层 `page_enter` 是确定高度的盒子 —— auto 高度解析成 0，`absolute().inset_0()`
//! 的遮罩塌缩，对话框以零高盒居中，整个表单被推到视口上方（「所属课程」在 y<0），
//! 于是「点不到课程、也存不了」。零安排恰好是新装后的第一个状态。
//!
//! 与其它视觉用例一致，默认跳过：`COURSEPLAN_VISUAL=1 cargo test --test schedule_flow`。

mod support;

use course_plan::app::Page;
use course_plan::gpui::component::ThemeMode;
use course_plan::gpui::test::TestWindowExt;
use course_plan::gpui::{
    AnyWindowHandle, InputEvent as _, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, point, px,
};
use course_plan::views::schedule::SessionDraft;

/// 用真实指针事件点某个逻辑坐标（`TestWindowExt::click` 只能点已注册 id 的元素）。
fn click_at(harness: &mut support::Harness, window: AnyWindowHandle, at: Point<Pixels>) {
    harness
        .cx
        .update_window(window, |_, window, cx| {
            for event in [
                MouseMoveEvent {
                    position: at,
                    pressed_button: None,
                    modifiers: Default::default(),
                }
                .to_platform_input(),
                MouseDownEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                MouseUpEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: Default::default(),
                    click_count: 1,
                }
                .to_platform_input(),
            ] {
                window.dispatch_event(event, cx);
                window.render_frame(cx);
            }
        })
        .expect("派发点击失败");
}

/// 取元素中心点（元素必须由 `test_support()` 登记）。
fn center(
    harness: &mut support::Harness,
    window: AnyWindowHandle,
    id: &'static str,
) -> Point<Pixels> {
    let bounds = harness
        .cx
        .update_window(window, |_, window, _| {
            window.try_find(id).map(|s| s.bounds())
        })
        .unwrap_or_else(|err| panic!("读取 {id} 边界失败：{err}"))
        .unwrap_or_else(|| panic!("渲染树里找不到 {id}"));
    point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    )
}

fn open_editor(harness: &mut support::Harness, window: AnyWindowHandle) {
    let app = harness.app.clone();
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

/// 零安排（有课程）：对话框完整落在视口内，课程可选，保存后新增一条安排。
#[test]
fn lane_v2_session_flow_without_existing_sessions() {
    if !support::enabled() {
        support::skip("lane_v2_session_flow_without_existing_sessions");
        return;
    }
    unsafe {
        std::env::set_var("COURSEPLAN_DATA_DIR", "/tmp/opencode/schedule-flow-data");
    }

    let mut db = course_plan::store::sample_data();
    db.sessions.clear(); // 零安排：新装后的第一个状态
    let mut harness = support::Harness::with_database(db);
    let window = harness.window;
    harness.set_page(Page::Schedule);
    harness.set_theme(ThemeMode::Light);
    harness.draw();

    let viewport = harness
        .cx
        .update_window(window, |_, window, _| window.viewport_size())
        .expect("读取视口失败");

    open_editor(&mut harness, window);
    harness.draw();

    // 回归点：零安排时对话框不能塌到视口外。
    let trigger = harness
        .cx
        .update_window(window, |_, window, _| {
            window
                .try_find("select-trigger:session-course")
                .map(|s| s.bounds())
        })
        .expect("读取课程下拉失败")
        .expect("零安排时「所属课程」下拉必须存在");
    assert!(
        trigger.origin.y > px(0.) && trigger.origin.y + trigger.size.height < viewport.height,
        "零安排时对话框被推出视口：课程下拉 y={:?}，视口高={:?}",
        trigger.origin.y,
        viewport.height
    );

    // 展开 → 选第一门课。
    let trigger_center = center(&mut harness, window, "select-trigger:session-course");
    click_at(&mut harness, window, trigger_center);
    harness.draw();
    let option_center = center(&mut harness, window, "select-option:session-course:0");
    click_at(&mut harness, window, option_center);
    harness.draw();

    let save_center = center(&mut harness, window, "session-save");
    click_at(&mut harness, window, save_center);
    harness.draw();

    let (count, editor_open, first_course) = harness.cx.update(|cx| {
        let app = harness.app.read(cx);
        (
            app.db.live_session_count(),
            app.pending_session.is_some(),
            app.db.live_sessions().next().map(|s| s.course_id.clone()),
        )
    });
    assert_eq!(count, 1, "保存后应新增一条安排");
    assert!(!editor_open, "保存后编辑器应关闭");
    assert!(
        first_course.is_some_and(|id| !id.is_empty()),
        "安排必须带 course_id"
    );
}

/// 零课程：不打开死表单，直接引导去「课程管理」。
#[test]
fn lane_v2_session_requires_a_course_first() {
    if !support::enabled() {
        support::skip("lane_v2_session_requires_a_course_first");
        return;
    }
    unsafe {
        std::env::set_var("COURSEPLAN_DATA_DIR", "/tmp/opencode/schedule-flow-empty");
    }

    let mut harness = support::Harness::with_database(course_plan::model::Database::default());
    let window = harness.window;
    harness.set_page(Page::Schedule);
    harness.set_theme(ThemeMode::Light);
    harness.draw();

    open_editor(&mut harness, window);

    let (editor_open, page) = harness.cx.update(|cx| {
        let app = harness.app.read(cx);
        (app.pending_session.is_some(), app.page)
    });
    assert!(!editor_open, "没有课程时不应打开空的排课表单");
    assert_eq!(page, Page::Courses, "应把用户带到「课程管理」去建课程");
}
