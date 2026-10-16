//! CoursePlan 库入口：导出全部模块，供二进制、集成测试与无头渲染测试使用。

pub mod app;
pub mod assets;
pub mod model;
pub mod prelude;
pub mod store;
pub mod sync;
pub mod ui;
pub mod views;

// GPUI Kit 可以作为 `gpui` 别名使用，这样 `gpui::*` 就能拿到全部 GPUI API。
// `pub` 让集成测试/无头渲染测试也可以通过 `course_plan::gpui` 使用（含 gpui::test）。
pub extern crate gpui_kit as gpui;

use app::{APP_ID, App};
use gpui::component::{Theme, ThemeMode};
use gpui::*;

/// 启动桌面应用（由 `src/main.rs` 调用；集成测试直接使用各模块，不调用此函数）。
pub fn run() {
    application().with_assets(assets::AppAssets).run(|cx| {
        init(cx);

        // 通知与桌面集成使用稳定的应用标识。
        cx.set_app_identity(APP_ID, "CoursePlan");
        Theme::change(ThemeMode::Light, None, cx);

        open_window(
            WindowOptions {
                window_min_size: Some(size(px(1000.), px(640.))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                window.resize(size(px(1280.), px(840.)));
                cx.new(|_| App::new())
            },
        )
        .expect("无法创建窗口");
    });
}
