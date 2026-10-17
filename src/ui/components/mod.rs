//! 设计系统组件库（详见 `docs/DESIGN.md` §3 与 §6）。
//!
//! 命名规范：
//! - 顶层公开 API 一律 `pub fn xxx(...) -> impl IntoElement`（gpui-kit 控件可直接返回 Button）。
//! - 内部 helper 用 `fn` 私有。
//!
//! 视图层调用前请先看 §6 的 API freeze——签名是契约。

pub mod button;
pub mod card;
pub mod check_circle;
pub mod empty_state;
pub mod kv_row;
pub mod list_row;
pub mod nav_item;
pub mod overlay;
pub mod page_header;
pub mod panel;
pub mod progress;
pub mod stat_tile;
pub mod sync_pill;
pub mod tab_chips;
pub mod tag;
