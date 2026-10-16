mod app;
mod model;
mod prelude;
mod store;
mod ui;
mod views;

// GPUI Kit 可以作为 `gpui` 别名使用，这样 `gpui::*` 就能拿到全部 GPUI API。
extern crate gpui_kit as gpui;

use app::{APP_ID, App};
use gpui::component::{Theme, ThemeMode};
use gpui::*;

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
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
