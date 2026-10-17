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
        AlarmClock,
        Archive,
        ArchiveRestore,
        BellRing,
        Bookmark,
        CalendarClock,
        CalendarDays,
        CalendarPlus,
        ChartColumn,
        Check,
        ChevronLeft,
        CircleAlert,
        CircleCheck,
        CircleCheckBig,
        CircleDashed,
        CircleX,
        Clock,
        Copy,
        Download,
        Ellipsis,
        EllipsisVertical,
        ExternalLink,
        Flame,
        FolderOpen,
        GraduationCap,
        GripVertical,
        Info,
        Link2,
        ListTodo,
        LogIn,
        LogOut,
        MessageSquare,
        NotebookPen,
        Pencil,
        PenLine,
        Pin,
        RefreshCw,
        RotateCcw,
        Save,
        Search,
        Sparkle,
        SquarePen,
        Star,
        StickyNote,
        Target,
        Trash,
        TrendingDown,
        TrendingUp,
        TriangleAlert,
        Upload,
        UserRound,
        Users,
        X,
    ]
);

/// 应用引用的全部图标。任何新增的图标引用都必须先登记进本清单，
/// 由 `tests/icons.rs` 遍历源码做兜底校验。
pub const APP_ICONS: &[IconName] = &[
    IconName::AlarmClock,
    IconName::Archive,
    IconName::ArchiveRestore,
    IconName::Bell,
    IconName::BellRing,
    IconName::BookOpen,
    IconName::Bookmark,
    IconName::CalendarClock,
    IconName::CalendarDays,
    IconName::CalendarPlus,
    IconName::ChartColumn,
    IconName::Check,
    IconName::ChevronDown,
    IconName::ChevronLeft,
    IconName::ChevronRight,
    IconName::ChevronUp,
    IconName::CircleAlert,
    IconName::CircleCheck,
    IconName::CircleCheckBig,
    IconName::CircleDashed,
    IconName::CircleX,
    IconName::Clock,
    IconName::Copy,
    IconName::Download,
    IconName::Ellipsis,
    IconName::EllipsisVertical,
    IconName::ExternalLink,
    IconName::Eye,
    IconName::EyeOff,
    IconName::Flame,
    IconName::FolderOpen,
    IconName::GraduationCap,
    IconName::GripVertical,
    IconName::Info,
    IconName::LayoutDashboard,
    IconName::Link2,
    IconName::ListTodo,
    IconName::LogIn,
    IconName::LogOut,
    IconName::MessageSquare,
    IconName::Moon,
    IconName::NotebookPen,
    IconName::Pencil,
    IconName::PenLine,
    IconName::Pin,
    IconName::Plus,
    IconName::RefreshCw,
    IconName::RotateCcw,
    IconName::Save,
    IconName::Search,
    IconName::Settings,
    IconName::Sparkle,
    IconName::SquarePen,
    IconName::Star,
    IconName::StickyNote,
    IconName::Sun,
    IconName::Target,
    IconName::Trash,
    IconName::TrendingDown,
    IconName::TrendingUp,
    IconName::TriangleAlert,
    IconName::Upload,
    IconName::UserRound,
    IconName::Users,
    IconName::X,
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
