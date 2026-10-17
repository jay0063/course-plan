//! CoursePlan 库入口：导出全部模块，供二进制、集成测试与无头渲染测试使用。
#![recursion_limit = "512"]

pub mod app;
pub mod assets;
pub mod model;
pub mod prelude;
pub mod store;
pub mod sync;
pub mod theme;
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

        // 组件库内置文案切中文（日历 / 日期选择器的月份与星期、通知按钮等）。
        // 必须在任何组件渲染之前调用；`set_locale` 是进程级设置（rust-i18n）。
        gpui::component::set_locale("zh-CN");

        // 通知与桌面集成使用稳定的应用标识。
        cx.set_app_identity(APP_ID, "CoursePlan");

        // 装载 CoursePlan 主题（papery warm light + cozy dark，详见 docs/DESIGN.md §2）。
        // 必须在 Theme::change 之前调用——init 会把 ThemeConfig 装入 theme.{light,dark}_theme，
        // 使 Theme::change 命中我们的配置而不是 gpui-component 默认值。
        theme::init(cx);

        // 进入浅色主题（init 已经写好了当前模式的 colors / tokens；这一步只是切换 mode 并
        // 应用对应 ThemeConfig，让首次 render 就拿到完整调色板）。
        Theme::change(ThemeMode::Light, None, cx);
        theme::apply_for_mode(cx);

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
