//! Empty state（详见 `docs/DESIGN.md` §3.11）。
//!
//! 三种 variant：Illustrated（整页空态）/ Compact（列表内）/ Celebratory（全部完成）。

use crate::gpui::assets::IconName;
use crate::gpui::component::Size;
use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::tokens::text as text_tokens;
use crate::ui::tokens::{SPACE_LG, SPACE_XL};

#[derive(Clone, Copy)]
pub enum EmptyVariant {
    /// 整页空态：容器 320×240，居中，icon 56px。
    Illustrated,
    /// 列表内空态：padding-y 24，icon 32px。
    Compact,
    /// 全部完成：Sparkle 56px + 右上角小星，强调"做得好"。
    Celebratory,
}

pub fn empty_state(
    theme: &Theme,
    variant: EmptyVariant,
    icon: IconName,
    title: impl Into<SharedString>,
    description: Option<impl Into<SharedString>>,
    action: Option<impl IntoElement>,
) -> impl IntoElement {
    let title = title.into();
    let description: Option<SharedString> = description.map(Into::into);

    // Celebratory 固定用 Sparkle（§3.11）；其余使用调用方图标。
    let icon = if matches!(variant, EmptyVariant::Celebratory) {
        IconName::Sparkle
    } else {
        icon
    };
    let icon_size = match variant {
        EmptyVariant::Illustrated | EmptyVariant::Celebratory => Size::Size(px(56.)),
        EmptyVariant::Compact => Size::Size(px(32.)),
    };
    let title_size = match variant {
        EmptyVariant::Illustrated => text_tokens::TITLE,
        EmptyVariant::Compact => text_tokens::BODY_EMPH,
        EmptyVariant::Celebratory => px(22.),
    };
    let title_weight = if matches!(variant, EmptyVariant::Celebratory) {
        FontWeight(700.0)
    } else {
        FontWeight(600.0)
    };
    let icon_color = match variant {
        EmptyVariant::Celebratory => theme.warning,
        _ => theme.muted_foreground,
    };

    let container_padding_y = match variant {
        EmptyVariant::Illustrated | EmptyVariant::Celebratory => px(40.),
        EmptyVariant::Compact => px(24.),
    };

    let mut col = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(SPACE_LG)
        .py(container_padding_y)
        .px(SPACE_XL);

    if matches!(
        variant,
        EmptyVariant::Illustrated | EmptyVariant::Celebratory
    ) {
        col = col.min_h(px(240.)).w(px(320.)).mx_auto().rounded(px(20.));
    }

    let mut icon_block = div()
        .relative()
        .text_color(icon_color)
        .child(Icon::new(icon).with_size(icon_size));
    if matches!(variant, EmptyVariant::Celebratory) {
        icon_block = icon_block.child(
            div()
                .absolute()
                .top(px(-12.))
                .right(px(-16.))
                .text_color(theme.warning)
                .child(Icon::new(IconName::Sparkle).with_size(Size::Size(px(20.)))),
        );
    }

    col = col.child(icon_block).child(
        div()
            .text_size(title_size)
            .font_weight(title_weight)
            .text_color(theme.foreground)
            .child(title),
    );

    if let Some(desc) = description {
        col = col.child(
            div()
                .text_size(text_tokens::BODY_SM)
                .text_color(theme.muted_foreground)
                .text_center()
                .child(desc),
        );
    }

    if let Some(action) = action {
        col = col.child(action);
    }

    col
}
