//! 主题衍生：把 gpui-kit 的 ThemeColor 字段转为我们设计语言中的语义别名。
//!
//! 字段映射详见 `docs/DESIGN.md` §2.1 末尾。

use crate::gpui::component::Theme;

/// 卡片/侧栏/顶栏/输入底色（亮色：#FFFCF5 暖米；暗色：#26221E 深棕）。
///
/// gpui-kit 没有独立 `panel` 字段；用 `popover` 作语义对应（也是 elevated surface）。
#[inline]
pub fn panel(theme: &Theme) -> crate::gpui::Hsla {
    theme.popover
}

/// 全屏底色（亮色：#FBF6EC 米纸；暗色：#1B1815 深棕纸）。
#[inline]
pub fn surface(theme: &Theme) -> crate::gpui::Hsla {
    theme.background
}

/// 输入框聚焦态边框（亮色：#D9C8A3 暖深；暗色：#5C5240 深棕边）。
#[inline]
pub fn border_strong(theme: &Theme) -> crate::gpui::Hsla {
    theme.input
}

/// 顶栏底色。
#[inline]
pub fn topbar(theme: &Theme) -> crate::gpui::Hsla {
    theme.popover
}
