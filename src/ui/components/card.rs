//! 卡片族（详见 `docs/DESIGN.md` §3.7）。
//!
//! 5 变体 —— Default / Flat / Outlined / Accent(color) / Hero。
//! 统一入口 [`card_with_theme`]：传入 `&Theme` 拿到正确颜色。

use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::components::panel as panel_helper;
use crate::ui::tokens::text as text_tokens;
use crate::ui::tokens::{RADIUS_LG, RADIUS_XL};

#[derive(Clone, Copy)]
pub enum CardVariant {
    /// 标准卡片：panel 底 + 1px border。
    Default,
    /// 嵌入父容器（无 bg、无圆角）。
    Flat,
    /// 强分组：surface 底 + 2px border。
    Outlined,
    /// 强调色块：accent.with_alpha(0.10) 底 + 1px accent.with_alpha(0.4) border。
    Accent(Hsla),
    /// dashboard 顶部 4 块统计卡：panel + 1px border + shadow_md + 圆角 xl。
    Hero,
}

/// 主题感知卡片。Phase 4 视图统一使用此函数。
pub fn card_with_theme(
    theme: &Theme,
    variant: CardVariant,
    title: Option<impl Into<SharedString>>,
    subtitle: Option<impl Into<SharedString>>,
    content: impl IntoElement,
) -> impl IntoElement {
    let title: Option<SharedString> = title.map(Into::into);
    let subtitle: Option<SharedString> = subtitle.map(Into::into);

    let mut card_div = div().flex().flex_col().gap(px(12.));

    match variant {
        CardVariant::Default => {
            // §3.7：hover 用 border_strong（ThemeColor.input）表达"被注意"，
            // 不抬升（DESIGN §2.9）。
            let hover_border = theme.input;
            card_div = card_div
                .bg(panel_helper::panel(theme))
                .rounded(RADIUS_LG)
                .hover(move |s| s.border_color(hover_border));
        }
        CardVariant::Flat => {}
        CardVariant::Outlined => {
            card_div = card_div.bg(theme.background).rounded(RADIUS_LG);
        }
        CardVariant::Accent(color) => {
            card_div = card_div
                .bg(color.with_alpha(0.10))
                .border_1()
                .border_color(color.with_alpha(0.4))
                .rounded(RADIUS_LG);
        }
        CardVariant::Hero => {
            card_div = card_div
                .bg(panel_helper::panel(theme))
                .rounded(RADIUS_XL)
                .shadow_md();
        }
    }

    if matches!(
        variant,
        CardVariant::Default | CardVariant::Hero | CardVariant::Outlined
    ) {
        let bordered = if matches!(variant, CardVariant::Outlined) {
            card_div.border_2()
        } else {
            card_div.border_1()
        };
        card_div = bordered.border_color(theme.border);
    }

    card_div = card_div.p(px(16.));

    let mut final_div = card_div;
    if let Some(t) = title {
        final_div = final_div.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(text_tokens::SUBTITLE)
                        .font_weight(FontWeight(600.0))
                        .text_color(theme.foreground)
                        .child(t),
                )
                .when_some(subtitle, |d, s| {
                    d.child(
                        div()
                            .text_size(text_tokens::BODY_SM)
                            .text_color(theme.muted_foreground)
                            .child(s),
                    )
                }),
        );
    }
    final_div.child(content)
}
