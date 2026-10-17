//! 同步状态 pill（详见 `docs/DESIGN.md` §3.4）。

use crate::gpui::TestSupportExt as _;
use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::sync::Status;
use crate::ui::tokens::{RADIUS_FULL, SYNC_PILL_H};

/// 顶栏用同步状态 pill（24px 高，胶囊）。
///
/// `id` 是稳定 test-support id（§6.8：`ui:topbar:sync-pill`）。
pub fn sync_pill(
    theme: &Theme,
    id: impl Into<ElementId>,
    status: &Status,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let (label, dot_color, bg) = match status {
        Status::Idle {
            last_sync_ms: Some(ms),
        } => {
            let time = chrono::DateTime::from_timestamp_millis(*ms)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
                .unwrap_or_default();
            (
                if time.is_empty() {
                    "已同步".to_string()
                } else {
                    format!("已同步 {time}")
                },
                theme.success,
                theme.success.with_alpha(0.16),
            )
        }
        Status::Idle { .. } => (
            "已同步".to_string(),
            theme.success,
            theme.success.with_alpha(0.16),
        ),
        Status::Syncing => (
            "同步中…".to_string(),
            theme.info,
            theme.info.with_alpha(0.16),
        ),
        Status::Offline => ("离线".to_string(), theme.muted_foreground, theme.muted),
        Status::LoggedOut => ("未登录".to_string(), theme.muted_foreground, theme.muted),
        Status::Error { .. } => (
            "同步出错".to_string(),
            theme.danger,
            theme.danger.with_alpha(0.16),
        ),
        Status::AuthFailed => (
            "登录失败".to_string(),
            theme.danger,
            theme.danger.with_alpha(0.16),
        ),
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(SYNC_PILL_H)
        .px(px(10.))
        .rounded(RADIUS_FULL)
        .bg(bg)
        .cursor_pointer()
        .hover(|s| s.bg(bg))
        .on_click(on_click)
        .child(
            div()
                .size(px(6.))
                .rounded_full()
                .bg(dot_color)
                .flex_shrink_0(),
        )
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight(500.0))
                .text_color(theme.foreground)
                .child(label),
        )
        .test_support()
}

/// 侧栏 footer 用紧凑指示（圆点 + 5-6 字），可点击跳转设置页。
///
/// `id` 是稳定 test-support id（§6.8：`ui:sidebar:sync-indicator`）。
pub fn sync_indicator_compact(
    theme: &Theme,
    id: impl Into<ElementId>,
    status: &Status,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let (label, dot_color) = match status {
        Status::Idle { .. } => ("已同步", theme.success),
        Status::Syncing => ("同步中…", theme.info),
        Status::Offline => ("离线", theme.muted_foreground),
        Status::LoggedOut => ("未登录", theme.muted_foreground),
        Status::Error { .. } | Status::AuthFailed => ("同步出错", theme.danger),
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(6.))
        .py(px(4.))
        .rounded(px(8.))
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .on_click(on_click)
        .child(
            div()
                .size(px(6.))
                .rounded_full()
                .bg(dot_color)
                .flex_shrink_0(),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .test_support()
}
