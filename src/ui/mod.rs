//! 设计系统（详见 `docs/DESIGN.md`）。
//!
//! 公开层级：
//! - [`tokens`]：尺寸常量（间距/圆角/高度/图标）。
//! - [`palette`]：课程 8 色调色板与 tint。
//! - [`motion`]：动效原语（reduce-motion 安全）。
//! - [`components`]：组件族（按钮/卡片/导航/同步状态/页头/列表行等）。
//! - [`form`]：表单辅助（field_row）。
//! - [`select`]：内联 Select 控件（保留兼容）。
//! - [`widgets`]：**薄壳兼容层**——views 不修改即可编译（Phase 4 逐步迁移）。

pub mod components;
pub mod form;
pub mod motion;
pub mod palette;
pub mod select;
pub mod tokens;
pub mod widgets;
