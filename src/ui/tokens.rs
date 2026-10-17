//! 设计系统 token 常量（详见 `docs/DESIGN.md` §2）。
//!
//! 所有视图层都应通过此模块获取尺寸常量。**严禁在视图层直接写魔法 px() 值**。

use crate::gpui::Pixels;
use crate::gpui::px;

// MARK: 间距

pub const SPACE_XS: Pixels = px(4.);
pub const SPACE_SM: Pixels = px(8.);
pub const SPACE_MD: Pixels = px(12.);
pub const SPACE_LG: Pixels = px(16.);
pub const SPACE_XL: Pixels = px(20.);
pub const SPACE_2XL: Pixels = px(24.);
pub const SPACE_3XL: Pixels = px(32.);

pub const RADIUS_SM: Pixels = px(6.);
pub const RADIUS_MD: Pixels = px(10.);
pub const RADIUS_LG: Pixels = px(14.);
pub const RADIUS_XL: Pixels = px(20.);
pub const RADIUS_2XL: Pixels = px(28.);
pub const RADIUS_FULL: Pixels = px(9999.);

// MARK: 控件高度

pub const BUTTON_H: Pixels = px(36.);
pub const ICON_BUTTON_H: Pixels = px(32.);
pub const INPUT_H: Pixels = px(36.);
pub const NAV_ITEM_H: Pixels = px(36.);
pub const SYNC_PILL_H: Pixels = px(24.);
pub const SIDEBAR_BRAND_H: Pixels = px(72.);
pub const SIDEBAR_FOOTER_H: Pixels = px(56.);
pub const TOPBAR_H: Pixels = px(56.);
pub const SIDEBAR_W: Pixels = px(256.);

// MARK: 图标尺寸

pub const ICON_XS: Pixels = px(12.);
pub const ICON_SM: Pixels = px(14.);
pub const ICON_MD: Pixels = px(18.);
pub const ICON_LG: Pixels = px(22.);
pub const ICON_XL: Pixels = px(32.);

// MARK: 字阶（DESIGN §2.5）

/// 字号 token。颜色/字重由使用处从 `Theme` 与 `FontWeight` 取——
/// **不要**新增 `tokens::color`：颜色是主题态，必须走 `&Theme` 字段。
pub mod text {
    use crate::gpui::Pixels;
    use crate::gpui::px;

    pub const CAPTION: Pixels = px(12.);
    pub const LABEL_SM: Pixels = px(13.);
    pub const BODY_SM: Pixels = px(14.);
    pub const BODY: Pixels = px(15.);
    pub const BODY_EMPH: Pixels = px(15.);
    pub const LABEL: Pixels = px(14.);
    pub const SUBTITLE: Pixels = px(17.);
    pub const TITLE: Pixels = px(20.);
    pub const DISPLAY: Pixels = px(28.);
    pub const STAT: Pixels = px(32.);
}

// MARK: 阴影 / 透明度

/// 课程色块填充透明度（亮色）。
pub const COURSE_TINT_ALPHA_LIGHT: f32 = 0.16;
/// 课程色块填充透明度（暗色）。
pub const COURSE_TINT_ALPHA_DARK: f32 = 0.22;

/// 顶部 sync pill 同步中脉动频率（ms）。
pub const SYNC_PULSE_MS: u64 = 1200;
