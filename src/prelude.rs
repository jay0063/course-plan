//! 应用内统一的导入集合，避免每个模块重复一长串 `use`。
//!
//! 注意：`Size` 不从这里导出，因为它会和 `gpui::*` 里的 `Size<Pixels>` 冲突，
//! 需要显式 `use gpui::component::Size;`。
pub use gpui::component::{ActiveTheme, Icon, Sizable, StyledExt, Theme};
pub use gpui::prelude::*;

/// 按给定透明度拷贝一份主题色。
pub trait WithAlpha {
    fn with_alpha(self, alpha: f32) -> gpui::Hsla;
}

impl WithAlpha for gpui::Hsla {
    fn with_alpha(self, alpha: f32) -> gpui::Hsla {
        gpui::Hsla { a: alpha, ..self }
    }
}
