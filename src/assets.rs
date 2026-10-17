//! 应用图标资产源。
//!
//! gpui-kit 的默认 [`Assets`](crate::gpui::assets::Assets) 只内嵌组件所需的 106 个图标。
//! 应用还用到了 8 个不在其中的图标；一旦某个图标缺失，`AssetSource::load` 会返回
//! `Err`，导致对应控件的整块 SVG 绘制失败（图标按钮「消失」）。这里用
//! [`icon_assets!`](crate::gpui::assets::icon_assets) 补齐这些额外图标，并与默认
//! 图标集组合，从而同时保留组件内置图标与本应用图标。

use std::borrow::Cow;

use crate::gpui::assets::{Assets as KitAssets, IconName, icon_assets};
use crate::gpui::{AssetSource, Result, SharedString};

// 应用额外用到、但不在 gpui-kit 默认图标集中的图标。
icon_assets!(
    ExtraIcons,
    [
        CalendarDays,
        Clock,
        Flame,
        GraduationCap,
        ListTodo,
        NotebookPen,
        Sparkle,
        Target,
        Trash,
    ]
);

/// 应用引用的全部图标。任何新增的图标引用都必须先登记进本清单，
/// 由 `tests/icons.rs` 遍历源码做兜底校验。
pub const APP_ICONS: &[IconName] = &[
    IconName::Bell,
    IconName::BookOpen,
    IconName::CalendarDays,
    IconName::ChevronDown,
    IconName::ChevronRight,
    IconName::ChevronUp,
    IconName::Clock,
    IconName::Eye,
    IconName::EyeOff,
    IconName::Flame,
    IconName::GraduationCap,
    IconName::LayoutDashboard,
    IconName::ListTodo,
    IconName::Moon,
    IconName::NotebookPen,
    IconName::Plus,
    IconName::Settings,
    IconName::Sparkle,
    IconName::Sun,
    IconName::Target,
    IconName::Trash,
];

/// 应用资产源：先查额外图标，再回退到 gpui-kit 默认图标集。
///
/// 回退保证组件内部使用的默认图标仍然可用；额外图标则补上默认集中缺失的部分。
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        KitAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = KitAssets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
