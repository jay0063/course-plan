//! 课程 8 色调色板（详见 `docs/DESIGN.md` §2.6）。
//!
//! 索引与 `model::Course.color` 对应——任意 `Course.color` 都应通过
//! [`palette_at_for`] 取色（Phase 4 起强制；`palette_at` 仅保留给无主题的
//! 兼容调用），避免视图层直接索引 [`COURSE_PALETTE`]。
//!
//! ## 设计层约束（防止 hue 漂移 bug 复发）
//!
//! **`gpui::Hsla.h` 是 `[0, 1)` 的归一化角度，不是度数**。`gpui::hsla()` 会把
//! `h` clamp 到 `[0, 1]`，所以 `hsla(206.0, ...)` 与 `hsla(1.0, ...)` 等价——
//! 会让所有色相坍缩到红/品红。HSL→RGB 的 `From<Hsla> for Rgba`（vendor
//! `gpui-pre-0.3.9/src/color.rs`）用 `(h * 6.0).floor()` 选分支，越界走 fallback
//! 分支就是红色。
//!
//! **设计层契约**：所有 hue 用**度数**表示（参考 `docs/DESIGN.md` §2.1 / §2.2 /
//! §2.6 的 token 表），代码必须通过 [`hsl_deg`] 构造 Hsla；
//! 严禁手写 `hsla(206.0, ...)` 或 `Hsla { h: 206.0, ... }`——`hsl_deg` 一旦改变
//! 行为就能拦截所有回归。

use crate::gpui::Hsla;
use crate::gpui::component::Theme;
use crate::theme::hex_color;

/// **唯一合法**的 Hsla 构造入口：用**度数**构造。
///
/// `h_deg` 应当已经归一化到 `[0, 360)`。本函数**不**做 clamp——`gpui::hsla()` 内部会
/// clamp 到 `[0, 1]`，所以非法输入（≥360 或负数）会让所有色相坍缩成红/品红。
///
/// 为防止回归：这里把 `h_deg` 归位到 `[0, 360)`（用 `mod + if`-equiv，const 兼容
/// 因为 `f32::rem_euclid` 在 Rust 1.99 还没稳定为 const）。
/// 调用方应自行保证输入在 `[0, 360)`；越界输入会被静默归位。
///
/// `s ∈ [0, 1]`、`l ∈ [0, 1]`、`a = 1.0`（不透明）。
///
/// 是 `const fn`，可在 `static` 与 `const` 中使用（如 `COURSE_PALETTE`）。
#[inline]
pub const fn hsl_deg(h_deg: f32, s: f32, l: f32) -> Hsla {
    let h_deg = normalize_deg(h_deg);
    Hsla {
        h: h_deg / 360.0,
        s,
        l,
        a: 1.0,
    }
}

/// 把角度归位到 `[0, 360)`（const 兼容实现）。
const fn normalize_deg(h_deg: f32) -> f32 {
    let r = h_deg % 360.0;
    if r < 0.0 { r + 360.0 } else { r }
}

/// 8 色：明亮糖果色，与米黄/深棕纸面协调（DESIGN.md §2.6 第一列）。
///
/// **hex 是契约**——本表用 `hex_color` 反推 Hsla，确保每次取色得到设计 hex。
/// `hex_color` 是 `const fn`，因此本表是编译期常量（无运行期初始化）。
pub const COURSE_PALETTE: [Hsla; 8] = [
    hex_color("#5AA9E6"), // 0 天蓝
    hex_color("#F38B6B"), // 1 蜜桃
    hex_color("#5DBB6A"), // 2 薄荷
    hex_color("#F0B73D"), // 3 芒果
    hex_color("#9D7DD6"), // 4 葡萄
    hex_color("#4DBFB3"), // 5 青瓷
    hex_color("#A6C84B"), // 6 嫩芽
    hex_color("#E36B95"), // 7 玫瑰
];

/// 暗色版调色板（DESIGN.md §2.6 第二列）。
pub const COURSE_PALETTE_DARK: [Hsla; 8] = [
    hex_color("#7FBFEE"), // 0 天蓝（亮）
    hex_color("#FFA38A"), // 1 蜜桃（亮）
    hex_color("#7CCE89"), // 2 薄荷（亮）
    hex_color("#FFC664"), // 3 芒果（亮）
    hex_color("#B79DE8"), // 4 葡萄（亮）
    hex_color("#74D6CB"), // 5 青瓷（亮）
    hex_color("#BCDC72"), // 6 嫩芽（亮）
    hex_color("#F08FB0"), // 7 玫瑰（亮）
];

/// 根据索引取色（环绕到合法范围）。
pub fn palette_at(index: usize) -> Hsla {
    COURSE_PALETTE[index % COURSE_PALETTE.len()]
}

/// 根据主题自动在亮/暗调色板间切换。
///
/// **Phase 4 必须使用此函数**（而不是 [`palette_at`]），否则暗色主题下课程色
/// 会错误地使用亮色调色板。
pub fn palette_at_for(index: usize, theme: &Theme) -> Hsla {
    let palette = if theme.is_dark() {
        &COURSE_PALETTE_DARK
    } else {
        &COURSE_PALETTE
    };
    palette[index % palette.len()]
}

/// 课程色块填充淡底（在亮/暗自适应的 alpha）。
pub fn tint(color: Hsla, theme: &Theme) -> Hsla {
    let alpha = if theme.is_dark() {
        crate::ui::tokens::COURSE_TINT_ALPHA_DARK
    } else {
        crate::ui::tokens::COURSE_TINT_ALPHA_LIGHT
    };
    Hsla { a: alpha, ..color }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::hsla_to_hex;
    use std::collections::HashSet;

    fn hex_to_rgb(s: &str) -> (u8, u8, u8) {
        let s = s.trim_start_matches('#');
        (
            u8::from_str_radix(&s[0..2], 16).unwrap(),
            u8::from_str_radix(&s[2..4], 16).unwrap(),
            u8::from_str_radix(&s[4..6], 16).unwrap(),
        )
    }

    fn assert_hex_close(label: &str, got: &str, expected: &str, tol: u8) {
        let (gr, gg, gb) = hex_to_rgb(got);
        let (er, eg, eb) = hex_to_rgb(expected);
        assert!(
            gr.abs_diff(er) <= tol && gg.abs_diff(eg) <= tol && gb.abs_diff(eb) <= tol,
            "{label}: got {got}, expected {expected} (±{tol} per channel); got=({gr},{gg},{gb}) expected=({er},{eg},{eb})"
        );
    }

    /// DESIGN.md §2.6 第一列：8 个浅色课程色 hex 匹配。
    /// 容忍度 ≤2 / 通道——hex 是设计契约；HSL 反算引入 ≤1 通道浮点漂移。
    #[test]
    fn light_palette_hex_matches_design() {
        let expected = [
            "#5AA9E6", // 0 天蓝
            "#F38B6B", // 1 蜜桃
            "#5DBB6A", // 2 薄荷
            "#F0B73D", // 3 芒果
            "#9D7DD6", // 4 葡萄
            "#4DBFB3", // 5 青瓷
            "#A6C84B", // 6 嫩芽
            "#E36B95", // 7 玫瑰
        ];
        for (i, exp) in expected.iter().enumerate() {
            assert_hex_close(
                &format!("COURSE_PALETTE[{i}]"),
                &hsla_to_hex(palette_at(i)),
                exp,
                2,
            );
        }
    }

    /// DESIGN.md §2.6 第二列：8 个暗色课程色 hex 匹配。
    #[test]
    fn dark_palette_hex_matches_design() {
        let expected = [
            "#7FBFEE", // 0 天蓝（亮）
            "#FFA38A", // 1 蜜桃（亮）
            "#7CCE89", // 2 薄荷（亮）
            "#FFC664", // 3 芒果（亮）
            "#B79DE8", // 4 葡萄（亮）
            "#74D6CB", // 5 青瓷（亮）
            "#BCDC72", // 6 嫩芽（亮）
            "#F08FB0", // 7 玫瑰（亮）
        ];
        for (i, exp) in expected.iter().enumerate() {
            assert_hex_close(
                &format!("COURSE_PALETTE_DARK[{i}]"),
                &hsla_to_hex(COURSE_PALETTE_DARK[i]),
                exp,
                2,
            );
        }
    }

    /// 暗色主题下 [`palette_at_for`] 必须返回暗色调色板（Gate 3: 暗色盘可达）。
    #[test]
    fn palette_at_for_switches_with_theme() {
        let light = Theme {
            mode: crate::gpui::component::ThemeMode::Light,
            ..Theme::default()
        };
        for (i, expected) in COURSE_PALETTE.iter().enumerate() {
            assert_eq!(palette_at_for(i, &light), *expected);
        }
        let dark = Theme {
            mode: crate::gpui::component::ThemeMode::Dark,
            ..Theme::default()
        };
        for (i, expected) in COURSE_PALETTE_DARK.iter().enumerate() {
            assert_eq!(palette_at_for(i, &dark), *expected);
        }
        assert_ne!(palette_at_for(6, &dark), COURSE_PALETTE[6]);
    }

    /// 8 个课程色 hue 必须各不相同——之前用 `hsla(206.0)` 等度数被 clamp 到 `[0,1]`
    /// 后坍缩成红色（h=1.0），所有色都变成同一个红/品红。修复后应至少有 6 个
    /// 不同的 hex 值（部分颜色 hex 经过 round-trip 可能漂移，但 hue 不会相同）。
    #[test]
    fn course_palette_hues_are_distinct() {
        // vendor 的 `From<Hsla> for Rgba` 用 `(h * 6).floor()` 选分支，
        // 一共 6 个分支（h ∈ [0,1)）。多个 hue 可能落入同一分支（视觉颜色不同但走同一
        // 算法路径），所以严格分支检查是 ≥6 而不是 8。
        let hexes_light: HashSet<String> = (0..8).map(|i| hsla_to_hex(palette_at(i))).collect();
        let hexes_dark: HashSet<String> = (0..8)
            .map(|i| hsla_to_hex(COURSE_PALETTE_DARK[i]))
            .collect();
        eprintln!("light hexes: {hexes_light:?}");
        eprintln!("dark hexes:  {hexes_dark:?}");
        assert!(
            hexes_light.len() >= 6,
            "expected ≥6 distinct light hex values, got {}: {:?}",
            hexes_light.len(),
            hexes_light
        );
        assert!(
            hexes_dark.len() >= 6,
            "expected ≥6 distinct dark hex values, got {}: {:?}",
            hexes_dark.len(),
            hexes_dark
        );
    }

    /// 防御性：`hsl_deg` 不会因为度数 clamp 而坍缩到红/品红。
    #[test]
    fn hsl_deg_preserves_hue() {
        // 关键案例：degrees 模式下常用的大角度不能被 clamp 到 1.0
        let cases = [
            (206.0, 0.74, 0.625, "天蓝"),
            (14.0, 0.85, 0.685, "蜜桃"),
            (132.0, 0.45, 0.545, "薄荷"),
            (40.0, 0.85, 0.590, "芒果"),
            (264.0, 0.55, 0.665, "葡萄"),
            (174.0, 0.52, 0.520, "青瓷"),
            (76.0, 0.55, 0.535, "嫩芽"),
            (340.0, 0.65, 0.655, "玫瑰"),
        ];
        for (h_deg, s, l, label) in cases {
            let c = hsl_deg(h_deg, s, l);
            // h 应等于 h_deg / 360.0（不被 clamp）
            let expected = h_deg / 360.0;
            let diff = (c.h - expected).abs();
            assert!(
                diff < 1e-6,
                "{label}: hsl_deg({h_deg}, {s}, {l}) clamp 到 h={} (期望 h={})",
                c.h,
                expected
            );
        }
    }
}
