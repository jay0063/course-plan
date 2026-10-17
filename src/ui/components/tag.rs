//! Tag / Badge / Chip（详见 `docs/DESIGN.md` §3.6）。
//!
//! **对比度策略**：彩色填充上的文字不写死白色，而是按 WCAG 相对亮度在
//! 「暖深墨」与「纸白」之间选对比度更高的一侧；半透明 tint 上的文字先用
//! accent 本身，若 <4.5 再沿同一色相加深/提亮，最后退回中性色/纯黑纯白兜底。
//! 这样浅色主题的芒果黄/薄荷绿不会再出现 2:1 的白字，任意中灰（如
//! `#777777`，暖墨/纸白都只有 ~4.4）也能保证 ≥4.5。

use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::theme::{contrast_ratio, rgb_to_hsla};

/// 暖纸主题的「墨」色（比纯黑柔和，但在任何浅底上仍 ≥4.5）。
fn ink() -> Hsla {
    Hsla {
        h: 30.0 / 360.0,
        s: 0.16,
        l: 0.07,
        a: 1.0,
    }
}

/// 纸白。
fn paper() -> Hsla {
    Hsla {
        h: 40.0 / 360.0,
        s: 0.30,
        l: 0.99,
        a: 1.0,
    }
}

/// 半透明 `over` 叠在不透明 `under` 上的近似复合色（sRGB 混合，足够选文字色）。
pub(crate) fn composite(over: Hsla, under: Hsla) -> Hsla {
    let (fr, fg, fb) = hsl_to_rgb(over.h, over.s, over.l);
    let (br, bg, bb) = hsl_to_rgb(under.h, under.s, under.l);
    let a = over.a.clamp(0.0, 1.0);
    let to_u8 = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    rgb_to_hsla(
        to_u8(fr * a + br * (1.0 - a)),
        to_u8(fg * a + bg * (1.0 - a)),
        to_u8(fb * a + bb * (1.0 - a)),
        255,
    )
}

/// 纯黑 / 纯白兜底：对**任意**背景色，二者中至少一个 ≥4.58:1
/// （WCAG 相对亮度数学下界：max(black, white) 的最小值 ≈4.58）。
///
/// 暖墨（`ink`）与纸白（`paper`）在中灰底上可能都不到 4.5
/// （如 `#777777`：墨 4.20、纸 4.41），此时必须用纯黑/纯白。
fn pure_extreme(background: Hsla) -> Hsla {
    let black = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0,
        a: 1.0,
    };
    let white = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 1.0,
    };
    if contrast_ratio(white, background) >= contrast_ratio(black, background) {
        white
    } else {
        black
    }
}

/// 在给定背景上选一个 ≥4.5 的文字色。
///
/// 优先保留 `preferred`（通常是 accent 本身）；不够时沿同色相加深/提亮；
/// 再退回暖墨/纸白；两者都不足时用纯黑/纯白兜底——因此对任意背景都满足 AA
/// （本函数用于 tag/chip 的半透明 tint，tint 已与底色复合）。
pub(crate) fn readable_text(background: Hsla, preferred: Hsla) -> Hsla {
    if contrast_ratio(preferred, background) >= 4.5 {
        return preferred;
    }
    let (dark_ink, light_paper) = (ink(), paper());
    let prefer_dark =
        contrast_ratio(dark_ink, background) >= contrast_ratio(light_paper, background);
    let direction = if prefer_dark { -1.0 } else { 1.0 };
    for step in 1..=12 {
        let candidate = Hsla {
            l: (preferred.l + direction * 0.05 * step as f32).clamp(0.0, 1.0),
            ..preferred
        };
        if contrast_ratio(candidate, background) >= 4.5 {
            return candidate;
        }
    }
    let neutral = if prefer_dark { dark_ink } else { light_paper };
    if contrast_ratio(neutral, background) >= 4.5 {
        neutral
    } else {
        pure_extreme(background)
    }
}

/// 实心填充上的白/墨二选一（取对比度更高者）。
///
/// 暖墨/纸白不足 4.5 时（中灰 fill）退回纯黑/纯白，保证任意 fill 都 ≥4.5。
pub(crate) fn text_on_fill(fill: Hsla) -> Hsla {
    let (warm_ink, warm_paper) = (ink(), paper());
    let neutral = if contrast_ratio(warm_paper, fill) >= contrast_ratio(warm_ink, fill) {
        warm_paper
    } else {
        warm_ink
    };
    if contrast_ratio(neutral, fill) >= 4.5 {
        neutral
    } else {
        pure_extreme(fill)
    }
}

/// 圆角胶囊标签：用于卡片标题旁分类（"补习" / "校外"）。
///
/// 22px 高，padding-x 10px，bg = accent.with_alpha(0.18)，文字 12/600。
/// 文字色自动保证在 tint 上 ≥4.5（优先 accent 本身）。
pub fn tag(theme: &Theme, text: impl Into<SharedString>, accent: Hsla) -> impl IntoElement {
    let text = text.into();
    let tint = accent.with_alpha(0.18);
    let effective_bg = composite(tint, theme.popover);
    div()
        .flex()
        .items_center()
        .px(px(10.))
        .h(px(22.))
        .rounded_full()
        .bg(tint)
        .text_xs()
        .font_weight(FontWeight(600.0))
        .text_color(readable_text(effective_bg, accent))
        .child(text)
}

/// 圆形徽章：用于导航 / 计数，18×18 圆点风格。
///
/// 文字 11/700；文字色按 fill 亮度在白/墨之间自动选择，保证 WCAG AA。
pub fn badge(count: usize, accent: Hsla) -> impl IntoElement {
    let label = if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    };
    div()
        .min_w(px(18.))
        .h(px(18.))
        .px(px(5.))
        .rounded_full()
        .bg(accent)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.))
        .font_weight(FontWeight(700.0))
        .text_color(text_on_fill(accent))
        .child(label)
}

/// Chip：更小的内联标签（20px 高）。
pub fn chip(theme: &Theme, text: impl Into<SharedString>, accent: Hsla) -> impl IntoElement {
    let text = text.into();
    let tint = accent.with_alpha(0.16);
    let effective_bg = composite(tint, theme.popover);
    div()
        .flex()
        .items_center()
        .px(px(8.))
        .h(px(20.))
        .rounded_full()
        .bg(tint)
        .text_size(px(11.))
        .font_weight(FontWeight(600.0))
        .text_color(readable_text(effective_bg, accent))
        .child(text)
}

/// 旧版 `chip` 兼容（带 filled 选项）——views 已调用
/// `chip("...", color, true|false)`。filled=true 时为实心，false 为边框样式。
///
/// 实心填充文字不再写死白色：芒果黄等浅 fill 会自动改用深墨（见 §5.1）。
pub fn chip_with_filled(
    text: impl Into<SharedString>,
    color: Hsla,
    filled: bool,
) -> impl IntoElement {
    let text = text.into();
    let label_fg = if filled { text_on_fill(color) } else { color };
    let mut d = div().px(px(8.)).py(px(2.)).rounded_full().text_xs();
    if filled {
        d = d.bg(color).text_color(label_fg);
    } else {
        d = d.border_1().border_color(color).text_color(color);
    }
    d.child(text)
}

/// 局部 HSL → RGB（与 `theme::hsl_to_rgb` 的 vendor 算法一致；该函数私有，
/// 这里保留一份以便 composite 使用）。
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let cm = c + m;
    let xm = x + m;
    let (r, g, b) = match (h * 6.0).floor() as i32 {
        0 | 6 => (cm, xm, m),
        1 => (xm, cm, m),
        2 => (m, cm, xm),
        3 => (m, xm, cm),
        4 => (xm, m, cm),
        _ => (cm, m, xm),
    };
    (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::{composite, readable_text, text_on_fill};
    use crate::gpui::Hsla;
    use crate::prelude::WithAlpha as _;
    use crate::theme::{contrast_ratio, dark_palette, hex_color, light_palette};

    fn assert_aa(label: &str, fg: Hsla, bg: Hsla) {
        let cr = contrast_ratio(fg, bg);
        assert!(cr >= 4.5, "{label}: cr={cr:.2}（期望 ≥4.5）");
    }

    /// 浅/暗两套主题的语义 fill 上，badge 自动文字必须 AA。
    #[test]
    fn badge_text_meets_aa_on_semantic_fills() {
        for (mode, palette) in [("light", light_palette()), ("dark", dark_palette())] {
            for (name, fill) in [
                ("danger", palette.danger),
                ("warning", palette.warning),
                ("success", palette.success),
                ("accent", palette.accent),
                ("primary", palette.primary),
            ] {
                let fg = text_on_fill(fill);
                assert_aa(&format!("{mode} badge on {name}"), fg, fill);
            }
        }
    }

    /// Gate 3 特别点名的组合（浅色 danger/warning + 暗色 danger/warning）。
    #[test]
    fn badge_text_meets_aa_on_danger_and_warning() {
        for (mode, danger, warning) in [
            ("light", "#9C2929", "#9A650F"),
            ("dark", "#F08B8B", "#FFC664"),
        ] {
            for (name, hex) in [("danger", danger), ("warning", warning)] {
                let fill = hex_color(hex);
                assert_aa(&format!("{mode} badge on {name}"), text_on_fill(fill), fill);
            }
        }
    }

    /// tag 的文字色在 18% tint 上必须 AA（浅色 info/warning 是历史失败点）。
    #[test]
    fn tag_text_meets_aa_on_tints() {
        for (mode, palette) in [("light", light_palette()), ("dark", dark_palette())] {
            for (name, accent) in [
                ("info", palette.info),
                ("warning", palette.warning),
                ("success", palette.success),
                ("danger", palette.danger),
                ("accent", palette.accent),
            ] {
                let bg = composite(accent.with_alpha(0.18), palette.popover);
                assert_aa(
                    &format!("{mode} tag on {name} tint"),
                    readable_text(bg, accent),
                    bg,
                );
            }
        }
    }

    /// 实心 chip 的文字色必须 AA（历史问题：浅色芒果黄上写死白字）。
    #[test]
    fn filled_chip_text_meets_aa_including_light_fills() {
        for hex in [
            "#F0B73D", "#5DBB6A", "#5AA9E6", "#9C2929", "#3D8246", "#9A650F",
        ] {
            let fill = hex_color(hex);
            assert_aa(&format!("filled chip on {hex}"), text_on_fill(fill), fill);
        }
    }

    /// 中灰 fill（暖墨/纸白都不足 4.5）必须由纯黑/纯白兜底，见 §5.1。
    ///
    /// `#777777`：暖墨 4.20、纸白 4.41——旧实现在这里会输出 <4.5 的文字。
    #[test]
    fn text_on_fill_meets_aa_on_midtone_fills() {
        for hex in ["#777777", "#808080", "#8A8A8A", "#6F6F6F"] {
            let fill = hex_color(hex);
            let fg = text_on_fill(fill);
            assert_aa(&format!("text_on_fill({hex})"), fg, fill);
        }
        // 只有暖墨/纸白都不足时才会落到纯黑/纯白。
        let mid = hex_color("#777777");
        let fg = text_on_fill(mid);
        assert!(
            (fg.l == 0.0 || fg.l == 1.0) && fg.s == 0.0,
            "#777777: 兜底应为纯黑/纯白，got l={} s={}",
            fg.l,
            fg.s
        );
    }

    /// `readable_text` 对任意中灰背景也必须 AA（tag/chip 的 tint 复合后可能落在这里）。
    #[test]
    fn readable_text_meets_aa_on_midtone_backgrounds() {
        for bg_hex in ["#777777", "#808080", "#8A8A8A"] {
            let bg = hex_color(bg_hex);
            for preferred_hex in ["#F0B73D", "#5AA9E6", "#FFFFFF"] {
                let fg = readable_text(bg, hex_color(preferred_hex));
                assert_aa(
                    &format!("readable_text({preferred_hex}) on {bg_hex}"),
                    fg,
                    bg,
                );
            }
        }
    }
}
