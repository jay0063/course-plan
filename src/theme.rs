//! 应用主题（CoursePlan 专属调色与字体策略）。
//!
//! 本文件是 `docs/DESIGN.md` 第 §2 节的**代码落地**。
//!
//! ## 机制（关键）
//!
//! **`Theme::change(mode)` 会从 `theme.{light,dark}_theme` 字段读取 `ThemeConfig`
//! 并 `apply_config`**（vendor `gpui-component-0.7.3/src/theme/mod.rs` 的 `edit`
//! 函数：reload_mode=true 时强制 apply）。如果我们只写 `theme.colors` 而不动
//! `theme.light_theme / theme.dark_theme`，任何外部 `Theme::change` 调用（例如
//! 设置页直接调用、测试 harness 切换主题、gpui-component 自身观察器触发的隐式切换）
//! 都会用 gpui-component 默认 ThemeConfig 把我们辛苦写好的颜色覆盖回去。
//!
//! 因此 [`init`] 的核心动作是：
//!
//! 1. 构造 `ThemeConfig { colors: theme_config_colors_from(<our palette>) }` × 2（亮/暗）
//! 2. **安装到 `theme.light_theme` / `theme.dark_theme`**——任何后续 `Theme::change`
//!    都会加载我们的 `ThemeConfig` 而不是 gpui-component 默认值。
//! 3. 立即 `apply_for_mode(cx)` 写 `theme.colors / radius / font / motion`，
//!    保证首帧渲染就拿到完整调色板（即使还没人调过 `Theme::change`）。
//!
//! [`apply_for_mode`] 是 belt-and-suspenders：直接写 `theme.colors`，跳过
//! JSON 解析，性能更好；但因为我们**也**已安装了正确的 `light_theme / dark_theme`，
//! 即使调用方省略 `apply_for_mode`（直接 `Theme::change`），也不会丢色。
//!
//! - 入口 [`init`] 在 `lib.rs::run()` 与 `tests/support::Harness::new()`
//!   都调用——幂等，重复调用直接返回。
//!
//! - 切换主题：调用 [`toggle_mode`]（封装 `Theme::change` + 再次 `apply_for_mode`）；
//!   或者直接 `Theme::change` 也能命中已安装的 CoursePlan ThemeConfig。
//!
//! - 不使用 `ThemeRegistry::load_themes_from_str` 注册 JSON 主题——因为
//!   `Theme::edit` 读的是 `theme.{light,dark}_theme` 字段，不是 registry，
//!   registry 路径在这里是误导。详见 §"Defect fix history"（git log）。

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::gpui::base::{Easing, Spring};
use crate::gpui::component::{
    Colorize as _, MotionTokens, Theme, ThemeColor, ThemeConfig, ThemeConfigColors, ThemeMode,
};
use crate::gpui::{App, Hsla, SharedString, Window};

use crate::prelude::WithAlpha;

/// 我们的 ThemeConfig name —— 注册到 ThemeRegistry 后，Theme::change
/// 会按名字找到对应配置。
pub const LIGHT_THEME_NAME: &str = "CoursePlan Light";
pub const DARK_THEME_NAME: &str = "CoursePlan Dark";

/// 默认字体：跟随系统（与 Phase 2 一致）。
pub const FONT_FAMILY: &str = ".SystemUIFont";

/// 默认基础字号：15（比 gpui 默认 16 略小，更紧凑——家长每日使用）。
pub const FONT_SIZE: f32 = 15.0;

/// 主圆角（影响 Button/Input/TabBar/Select 等所有 gpui-kit 控件）。
pub const RADIUS: f32 = 10.0;
/// 大圆角（影响 Dialog/Sheet/Notification 等大容器）。
pub const RADIUS_LG: f32 = 14.0;

/// 已经初始化过（防止 run() / Harness::new() 多次调用）。
///
/// 注意：单元测试用 [`init_for_test`] 重置，避免全局 flag 跨测试污染。
static INIT: AtomicBool = AtomicBool::new(false);

/// 装载 CoursePlan 主题。幂等：多次调用安全。
///
/// 调用时机：
/// - `lib.rs::run()` 内 `gpui::init(cx)` 之后；
/// - `tests/support::Harness::new()` 内 `gpui::init(cx)` 之后。
///
/// **关键步骤**：把我们的 ThemeConfig 安装到 `theme.light_theme` / `theme.dark_theme`，
/// 这样后续任何 `Theme::change(mode)` 都会加载我们的配置，而不是 gpui-component 默认。
/// 详见模块顶部文档。
pub fn init(cx: &mut App) {
    if INIT.swap(true, Ordering::SeqCst) {
        // 已初始化过——仍要补一次 `apply_for_mode` 以防全局被 reset，但不要重装
        // ThemeConfig（rc 持有，我们的安装依然有效）。
        apply_for_mode(cx);
        return;
    }

    install_theme_configs(cx);
}

/// 内部 helper：完整安装 ThemeConfig 并 apply_for_mode（不受 INIT flag 控制）。
///
/// 单元测试用它强制重新安装——全局 INIT flag 跨测试持有会导致后续测试看到的
/// 是 gpui-component 默认 ThemeConfig。
fn install_theme_configs(cx: &mut App) {
    let light = Rc::new(build_config(ThemeMode::Light));
    let dark = Rc::new(build_config(ThemeMode::Dark));

    // 1) 安装 ThemeConfig——任何后续 Theme::change 都会用我们的 palette。
    Theme::update(cx, |theme| {
        theme.light_theme = light.clone();
        theme.dark_theme = dark.clone();
    });

    // 2) 立即把 colors / radius / font / motion 写入 Theme 全局，
    //    保证首帧渲染（不依赖 Theme::change）就是 CoursePlan 调色板。
    apply_for_mode(cx);
}

/// 测试专用入口：绕过 INIT flag，强制重新安装 CoursePlan ThemeConfig + apply。
///
/// 单元测试每个测试用例的 TestAppContext 是独立的，但 `static INIT: AtomicBool`
/// 是进程级单例；测试 1 init 后 INIT=true，测试 2 再调 `init` 会 early-return。
/// 但如果测试 1 与测试 2 在不同 TestAppContext 中，Theme 全局各自独立，所以
/// 测试 2 看到的是 gpui-component 默认 ThemeConfig（因为没装我们的）。
///
/// 测试场景需要 `force_init_for_test(cx)`：仍然幂等，且每次都重新装。
#[cfg(test)]
pub fn force_init_for_test(cx: &mut App) {
    install_theme_configs(cx);
    // 重置 flag（仅在测试编译时）——允许后续 lib init 仍然有"未初始化"的语义。
    INIT.store(false, Ordering::SeqCst);
}

/// 构造一份与 `ThemeMode` 关联的 ThemeConfig（colors 用我们的 palette）。
///
/// 也写入 `font.family` / `radius` / `radius.lg` / `shadow`，避免 gpui-component
/// 默认值污染。
fn build_config(mode: ThemeMode) -> ThemeConfig {
    let colors = if mode.is_dark() {
        dark_palette()
    } else {
        light_palette()
    };
    ThemeConfig {
        is_default: true,
        name: if mode.is_dark() {
            SharedString::from(DARK_THEME_NAME)
        } else {
            SharedString::from(LIGHT_THEME_NAME)
        },
        mode,
        font_size: Some(FONT_SIZE),
        font_family: Some(SharedString::from(FONT_FAMILY)),
        mono_font_family: None, // 使用 gpui-component 默认（系统等宽）
        mono_font_size: None,   // 使用 gpui-component 默认（13）
        radius: Some(RADIUS as usize),
        radius_lg: Some(RADIUS_LG as usize),
        shadow: Some(true),
        colors: theme_config_colors_from(&colors),
        highlight: None,
    }
}

/// 把我们的 ThemeColor 转换为 ThemeConfigColors。
///
/// `ThemeConfigColors` 末尾有 12 个私有字段（`base.red..base.cyan.light`），
/// 只能在 crate 内构造。因此这里用 `ThemeConfigColors::default()` + 公共字段赋值；
/// 私有基础色保持 `None`（`apply_config` 只用它们做缺失字段的 fallback，而本函数
/// 把每个公共字段都显式填满）。
///
/// **alpha 必须保留**：`overlay`、`scrollbar_thumb`、`drop_target`、
/// `list_active(_border)`、`selection`、`table_active(_border)` 都带透明度。
/// 这里统一用 [`hsla_to_hex_full`] 输出 `#RRGGBBAA`；否则直接 `Theme::change`
/// 会把对话框遮罩变成不透明色块。
///
/// 注意 vendor 键名：进度条是 `progress.bar.background`（不是 `progress_bar`）。
/// 键集合由 `theme_config_keys_match_vendor_schema` 测试对
/// `serde_json::to_value(ThemeConfigColors::default())` 锁定。
fn theme_config_colors_from(colors: &ThemeColor) -> ThemeConfigColors {
    let mut cfg = ThemeConfigColors::default();

    macro_rules! set_colors {
        ($($field:ident),+ $(,)?) => {
            $(
                cfg.$field = Some(SharedString::from(hsla_to_hex_full(colors.$field)));
            )+
        };
    }

    // 与 vendor `ThemeConfigColors` 的公共字段一一对应（含 chart.1..5）。
    set_colors!(
        accent,
        accent_foreground,
        accordion,
        background,
        border,
        button,
        button_active,
        button_foreground,
        button_hover,
        button_danger,
        button_danger_active,
        button_danger_foreground,
        button_danger_hover,
        button_info,
        button_info_active,
        button_info_foreground,
        button_info_hover,
        button_primary,
        button_primary_active,
        button_primary_foreground,
        button_primary_hover,
        button_secondary,
        button_secondary_active,
        button_secondary_foreground,
        button_secondary_hover,
        button_success,
        button_success_active,
        button_success_foreground,
        button_success_hover,
        button_warning,
        button_warning_active,
        button_warning_foreground,
        button_warning_hover,
        group_box,
        group_box_foreground,
        caret,
        chart_1,
        chart_2,
        chart_3,
        chart_4,
        chart_5,
        chart_bullish,
        chart_bearish,
        chart_grid,
        danger,
        danger_active,
        danger_foreground,
        danger_hover,
        description_list_label,
        description_list_label_foreground,
        drag_border,
        drop_target,
        foreground,
        info,
        info_active,
        info_foreground,
        info_hover,
        input,
        link,
        link_active,
        link_hover,
        list,
        list_active,
        list_active_border,
        list_even,
        list_head,
        list_hover,
        muted,
        muted_foreground,
        popover,
        popover_foreground,
        primary,
        primary_active,
        primary_foreground,
        primary_hover,
        progress_bar,
        ring,
        scrollbar,
        scrollbar_thumb,
        scrollbar_thumb_hover,
        secondary,
        secondary_active,
        secondary_foreground,
        secondary_hover,
        selection,
        sidebar,
        sidebar_accent,
        sidebar_accent_foreground,
        sidebar_border,
        sidebar_foreground,
        sidebar_primary,
        sidebar_primary_foreground,
        skeleton,
        slider_bar,
        slider_thumb,
        success,
        success_foreground,
        success_hover,
        success_active,
        switch,
        switch_thumb,
        tab,
        tab_active,
        tab_active_foreground,
        tab_bar,
        tab_bar_segmented,
        tab_foreground,
        table,
        table_active,
        table_active_border,
        table_even,
        table_head,
        table_head_foreground,
        table_foot,
        table_foot_foreground,
        table_hover,
        table_row_border,
        title_bar,
        title_bar_border,
        status_bar,
        status_bar_border,
        warning,
        warning_active,
        warning_hover,
        warning_foreground,
        overlay,
        window_border,
    );

    // `group_box.title.foreground` 在 ThemeColor 里没有独立字段，复用 foreground。
    cfg.group_box_title_foreground = Some(SharedString::from(hsla_to_hex_full(colors.foreground)));

    cfg
}

/// `Hsla` → `#RRGGBB` 十六进制字符串（丢弃 alpha；测试/调试用）。
///
/// 需要保留透明度时用 [`hsla_to_hex_full`]。
#[cfg(test)]
pub(crate) fn hsla_to_hex(h: Hsla) -> String {
    // HSL → RGB (0..1) → 8-bit → hex
    let (r, g, b) = hsl_to_rgb(h.h, h.s, h.l);
    format!(
        "#{:02X}{:02X}{:02X}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    )
}

/// **Hex → Hsla**：把 `#RRGGBB` 或 `#RRGGBBAA` 字符串反序列化为 Hsla。
///
/// 这与 vendor `gpui-pre-0.3.9/src/color.rs:194-222` 的 `From<Hsla> for Rgba`
/// 算法对齐但反向：RGB → HSL（按 max/min/delta 分支算 hue）。
///
/// **设计层契约**：所有 token 都应通过本函数从设计表的 hex 反推 Hsla——
/// hex 是真理源（DESIGN.md §2.1 / §2.2 / §2.6），HSL 是派生量。
pub const fn hex_color(s: &str) -> Hsla {
    // 跳过可选的 '#' 前缀。
    let bytes_full = s.as_bytes();
    let bytes: &[u8] = match bytes_full {
        [b'#', rest @ ..] => rest,
        _ => bytes_full,
    };
    // 解码 hex 字符（0-15）。内联为 const fn（闭包在 const fn 不允许）。
    const fn hex_val(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => 0,
        }
    }
    const fn two(a: u8, b: u8) -> u8 {
        (hex_val(a) << 4) | hex_val(b)
    }
    const fn one(a: u8) -> u8 {
        (hex_val(a) << 4) | hex_val(a)
    }
    // 支持 #rgb / #rgba / #rrggbb / #rrggbbaa
    let (r, g, b, a) = match bytes {
        [r0, g0, b0] => (one(*r0), one(*g0), one(*b0), 0xff),
        [r0, g0, b0, a0] => (one(*r0), one(*g0), one(*b0), one(*a0)),
        [r0, r1, g0, g1, b0, b1] => (two(*r0, *r1), two(*g0, *g1), two(*b0, *b1), 0xff),
        [r0, r1, g0, g1, b0, b1, a0, a1] => {
            (two(*r0, *r1), two(*g0, *g1), two(*b0, *b1), two(*a0, *a1))
        }
        _ => unreachable!(),
    };
    rgb_to_hsla(r, g, b, a)
}

/// RGB → HSL（vendor 算法反向；与 `From<Hsla> for Rgba` 对齐）。
pub const fn rgb_to_hsla(r: u8, g: u8, b: u8, a: u8) -> Hsla {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;

    let max = if rf > gf && rf > bf {
        rf
    } else if gf > bf {
        gf
    } else {
        bf
    };
    let min = if rf < gf && rf < bf {
        rf
    } else if gf < bf {
        gf
    } else {
        bf
    };
    let delta = max - min;

    let l = (max + min) / 2.0;

    let s = if delta == 0.0 {
        0.0
    } else if l < 0.5 {
        delta / (2.0 * l)
    } else {
        delta / (2.0 - 2.0 * l)
    };

    let h = if delta == 0.0 {
        0.0
    } else if max == rf {
        rem_euclid_6((gf - bf) / delta) / 6.0
    } else if max == gf {
        rem_euclid_6((bf - rf) / delta + 2.0) / 6.0
    } else {
        rem_euclid_6((rf - gf) / delta + 4.0) / 6.0
    };

    Hsla {
        h,
        s,
        l,
        a: a as f32 / 255.0,
    }
}

/// const 兼容的 `rem_euclid`（f32 在 Rust 1.99 还未稳定为 const）。
const fn rem_euclid_6(x: f32) -> f32 {
    let r = x - 6.0 * (x / 6.0).floor();
    if r < 0.0 { r + 6.0 } else { r }
}

/// 把 Hsla 编码为 `#RRGGBB`（alpha=1）或 `#RRGGBBAA`（alpha<1）。
///
/// 与 `try_parse_color` 一致：hex 接受 3/6 位（短）与 8 位（带 alpha）。
pub(crate) fn hsla_to_hex_full(h: Hsla) -> String {
    let (r, g, b) = hsl_to_rgb(h.h, h.s, h.l);
    let r8 = (r * 255.0).round() as u8;
    let g8 = (g * 255.0).round() as u8;
    let b8 = (b * 255.0).round() as u8;
    let a8 = (h.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    if a8 == 255 {
        format!("#{:02X}{:02X}{:02X}", r8, g8, b8)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", r8, g8, b8, a8)
    }
}

/// WCAG 相对亮度（sRGB → linear → Y）。
///
/// 算法参考 https://www.w3.org/TR/WCAG21/#dfn-relative-luminance。
pub(crate) fn relative_luminance(h: Hsla) -> f32 {
    fn chan(c: f32) -> f32 {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let (r, g, b) = hsl_to_rgb(h.h, h.s, h.l);
    0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b)
}

/// WCAG 对比度 = `(L_lighter + 0.05) / (L_darker + 0.05)`。
pub(crate) fn contrast_ratio(fg: Hsla, bg: Hsla) -> f32 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// HSL → sRGB（0..1）。**与 gpui-component `From<Hsla> for Rgba` 完全对齐**
/// （vendor `gpui-pre-0.3.9/src/color.rs` 的 `c / x / m / match (h*6).floor()` 算法），
/// 这样我们的 hex round-trip 与 `try_parse_color` 一致——后者也走同样的 vendor 算法。
///
/// **重要**：gpui 的 `Hsla::h` 是 `[0, 1)` 的归一化角度（不是 `[0, 360)`）。
/// 直接传度数会被 clamp 成 `1.0`，所有色相坍缩成红色。**必须**用
/// [`crate::ui::palette::hsl_deg`] 把度数先转成 `[0, 1)` 角度。
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

/// 把完整 CoursePlan 主题应用到当前 Theme 全局。
///
/// 直接写 `theme.colors / tokens / radius / font / motion`，跳过 JSON 解析路径。
/// 这是 belt-and-suspenders：与已安装的 `theme.{light,dark}_theme` 协同，保证
/// 主题永远是 CoursePlan 调色板。
///
/// 公开 API：测试 harness 与 `toggle_mode` 在 `Theme::change` 之后调用此函数。
pub fn apply_for_mode(cx: &mut App) {
    use crate::gpui::component::ThemeTokens;
    Theme::update(cx, |theme| {
        // 重新安装 ThemeConfig——以防调用方先 reset 了全局（例如 gpui-component
        // observer 触发）。这一步是修复 Phase 3 早期缺陷的关键。
        let new_light = Rc::new(build_config(ThemeMode::Light));
        let new_dark = Rc::new(build_config(ThemeMode::Dark));
        theme.light_theme = new_light;
        theme.dark_theme = new_dark;

        let colors = if theme.mode.is_dark() {
            dark_palette()
        } else {
            light_palette()
        };
        theme.tokens = ThemeTokens::from(colors);
        theme.colors = colors;
        theme.radius = gpui::px(RADIUS);
        theme.radius_lg = gpui::px(RADIUS_LG);
        theme.font_family = SharedString::from(FONT_FAMILY);
        theme.font_size = gpui::px(FONT_SIZE);
        theme.shadow = true;
        theme.motion = motion_tokens();
    });
}

/// 切换主题（在浅↔深之间翻转）。
///
/// `window` 仅用于触发对应窗口的刷新（Theme::update 会刷新所有窗口，
/// 参数保留以兼容 `Theme::change` 的接口形状）。
///
/// 调用方（顶栏主题切换按钮）应优先使用此函数。直接 `Theme::change` 也是
/// 安全的——`init`/`apply_for_mode` 已把 CoursePlan ThemeConfig 装入
/// `theme.light_theme` / `theme.dark_theme`，所以 `Theme::change` 会加载我们的
/// 配置而不是 gpui-component 默认。但 `apply_for_mode` 仍保证首帧（`render`
/// 期间）已经写入 `theme.colors`（避免任何时序 race）。
pub fn toggle_mode(window: Option<&mut Window>, cx: &mut App) {
    let target = if Theme::global(cx).mode.is_dark() {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    };
    Theme::change(target, window, cx);
    apply_for_mode(cx);
}

pub(crate) fn light_palette() -> ThemeColor {
    // 浅色主题（DESIGN.md §2.1）。
    // **所有 token 以 hex 为契约**——下面 `hex_color("#…")` 反推出 Hsla；
    // 测试断言 `hex_color(palette_token)` 等于设计表 hex，±2 通道。
    // 糖橙/成功/警示/危险/糖蓝 的 H/S/L 经 vendor RGB→HSL 反算后呈现
    // "candy paper" 视觉（已验证与 hex 表一致）。
    let surface = hex_color("#FBF6EC");
    let panel = hex_color("#FFFCF5");
    let sidebar = hex_color("#F5EBD9");
    let topbar = panel;
    let border = hex_color("#EADFC8");
    let border_strong = hex_color("#D9C8A3");
    let foreground = hex_color("#2A2620");
    let muted_foreground = hex_color("#7A6F60");
    let muted = hex_color("#F1E8D5");
    let primary = hex_color("#F08A36");
    let primary_hover = hex_color("#E27A28");
    let primary_active = hex_color("#C9661E");
    // primary_foreground 在浅色模式下用深棕 ink（与 foreground 冲突但更深，
    // 保证在饱和 primary 橙底上 ≥AA 4.5；测试验证 7.10）。
    let primary_foreground = hex_color("#1F1612");
    let secondary = hex_color("#FFF1DD");
    let secondary_foreground = hex_color("#5A4422");
    let accent = hex_color("#3478AC"); // 加深保证白字 AA（DESIGN §2.1 注：原 #5AA9E6 仅用于链接/徽标 tint）
    let accent_foreground = hex_color("#FFFFFF");
    // success / warning / danger 都在浅色下用深一档的 fill + 白字，保证 AA ≥4.5。
    let success = hex_color("#3D8246");
    let success_foreground = hex_color("#FFFFFF");
    let warning = hex_color("#8F6612");
    let warning_foreground = hex_color("#FFFFFF");
    let danger = hex_color("#9C2929");
    let danger_foreground = hex_color("#FFFFFF");
    let info = accent;
    let info_foreground = accent_foreground;
    let progress_bar = primary;
    let ring = hex_color("#F0B96D");
    // list_active / selection 是半透明橙（vendor apply_config 会把 list_active 的
    // alpha 上限压到 0.20、selection 压到 0.30；这里直接取上限内，保证
    // `apply_for_mode` 与直接 `Theme::change` 得到同一颜色）。
    let selection = primary.with_alpha(0.24);
    let list_hover = muted;
    let list_active = primary.with_alpha(0.20);

    ThemeColor {
        accent,
        accent_foreground,
        accordion: panel,
        background: surface,
        border,
        button: panel,
        button_active: muted,
        button_foreground: foreground,
        button_hover: muted,
        button_danger: danger,
        button_danger_active: danger.darken(0.10),
        button_danger_foreground: danger_foreground,
        button_danger_hover: danger.darken(0.05),
        button_info: info,
        button_info_active: info.darken(0.10),
        button_info_foreground: info_foreground,
        button_info_hover: info.darken(0.05),
        button_primary: primary,
        button_primary_active: primary_active,
        button_primary_foreground: primary_foreground,
        button_primary_hover: primary_hover,
        button_secondary: secondary,
        button_secondary_active: secondary.darken(0.06),
        button_secondary_foreground: secondary_foreground,
        button_secondary_hover: secondary.darken(0.03),
        button_success: success,
        button_success_active: success.darken(0.10),
        button_success_foreground: success_foreground,
        button_success_hover: success.darken(0.05),
        button_warning: warning,
        button_warning_active: warning.darken(0.10),
        button_warning_foreground: warning_foreground,
        button_warning_hover: warning.darken(0.05),
        success,
        success_active: success.darken(0.10),
        success_foreground,
        success_hover: success.darken(0.05),
        group_box: muted,
        group_box_foreground: foreground,
        caret: primary,
        chart_1: hex_color("#A9D5F2"),
        chart_2: hex_color("#5AA9E6"),
        chart_3: hex_color("#3F8FD0"),
        chart_4: hex_color("#2E73B5"),
        chart_5: hex_color("#1F5C97"),
        chart_bullish: success,
        chart_bearish: danger,
        chart_grid: border,
        danger,
        danger_active: danger.darken(0.10),
        danger_foreground,
        danger_hover: danger.darken(0.05),
        description_list_label: muted,
        description_list_label_foreground: muted_foreground,
        drag_border: accent,
        drop_target: accent.opacity(0.20),
        foreground,
        info,
        info_active: info.darken(0.10),
        info_foreground,
        info_hover: info.darken(0.05),
        input: border_strong,
        link: primary,
        link_active: primary_active,
        link_hover: primary_hover,
        list: panel,
        list_active,
        list_active_border: primary.with_alpha(0.50),
        list_even: panel,
        list_head: muted,
        list_hover,
        muted,
        muted_foreground,
        popover: panel,
        popover_foreground: foreground,
        primary,
        primary_active,
        primary_foreground,
        primary_hover,
        progress_bar,
        ring,
        scrollbar: surface,
        scrollbar_thumb: muted_foreground.opacity(0.50),
        scrollbar_thumb_hover: muted_foreground.opacity(0.80),
        secondary,
        secondary_active: secondary.darken(0.06),
        secondary_foreground,
        secondary_hover: secondary.darken(0.03),
        selection,
        sidebar,
        sidebar_accent: list_active,
        sidebar_accent_foreground: primary,
        sidebar_border: border,
        sidebar_foreground: foreground,
        sidebar_primary: primary,
        sidebar_primary_foreground: primary_foreground,
        skeleton: muted,
        slider_bar: primary,
        slider_thumb: primary_foreground,
        switch: muted,
        switch_thumb: panel,
        tab: surface,
        tab_active: panel,
        tab_active_foreground: foreground,
        tab_bar: panel,
        tab_bar_segmented: muted,
        tab_foreground: muted_foreground,
        table: panel,
        table_active: list_active,
        table_active_border: primary.with_alpha(0.50),
        table_even: panel,
        table_head: muted,
        table_head_foreground: foreground,
        table_foot: muted,
        table_foot_foreground: foreground,
        table_hover: list_hover,
        table_row_border: border,
        title_bar: topbar,
        title_bar_border: border,
        status_bar: topbar,
        status_bar_border: border,
        warning,
        warning_active: warning.darken(0.10),
        warning_foreground,
        warning_hover: warning.darken(0.05),
        overlay: foreground.opacity(0.32),
        window_border: border,
        red: danger,
        red_light: danger.with_alpha(0.20),
        green: success,
        green_light: success.with_alpha(0.20),
        blue: accent,
        blue_light: accent.with_alpha(0.20),
        yellow: warning,
        yellow_light: warning.with_alpha(0.20),
        magenta: hex_color("#E36B95"),
        magenta_light: hex_color("#F4B6CC"),
        cyan: accent,
        cyan_light: accent.with_alpha(0.20),
    }
}

pub(crate) fn dark_palette() -> ThemeColor {
    // 暗色主题（DESIGN.md §2.2）——所有 token 以 hex 为契约。
    let surface = hex_color("#1B1815");
    let panel = hex_color("#26221E");
    let sidebar = hex_color("#1F1B17");
    let topbar = panel;
    let border = hex_color("#3A342C");
    let border_strong = hex_color("#5C5240");
    let foreground = hex_color("#F2E9D6");
    let muted_foreground = hex_color("#A89A82");
    let muted = hex_color("#332D25");
    let primary = hex_color("#FFA55C");
    let primary_hover = hex_color("#FFB870");
    let primary_active = hex_color("#E27A28");
    let primary_foreground = hex_color("#1B1815"); // 深棕字 + 糖橙底 = AA
    let secondary = hex_color("#3A3328");
    let secondary_foreground = foreground;
    let accent = hex_color("#7FBFEE");
    let accent_foreground = hex_color("#0F1F2E");
    let success = hex_color("#7CCE89");
    let success_foreground = hex_color("#0E2410");
    let warning = hex_color("#FFC664");
    let warning_foreground = hex_color("#3D2A0F");
    let danger = hex_color("#F08B8B");
    let danger_foreground = hex_color("#2E0F0F");
    let info = accent;
    let info_foreground = accent_foreground;
    let progress_bar = primary;
    let ring = hex_color("#FFC080");
    // 同浅色：alpha 取 vendor 上限内（深棕暖光，不是纯橙）。
    let selection = hex_color("#5A3D1A").opacity(0.30);
    let list_hover = muted;
    let list_active = hex_color("#5A3D1A").opacity(0.20);

    ThemeColor {
        accent,
        accent_foreground,
        accordion: panel,
        background: surface,
        border,
        button: panel,
        button_active: muted,
        button_foreground: foreground,
        button_hover: muted,
        button_danger: danger,
        button_danger_active: danger.darken(0.10),
        button_danger_foreground: danger_foreground,
        button_danger_hover: danger.darken(0.05),
        button_info: info,
        button_info_active: info.darken(0.10),
        button_info_foreground: info_foreground,
        button_info_hover: info.darken(0.05),
        button_primary: primary,
        button_primary_active: primary_active,
        button_primary_foreground: primary_foreground,
        button_primary_hover: primary_hover,
        button_secondary: secondary,
        button_secondary_active: secondary.darken(0.06),
        button_secondary_foreground: secondary_foreground,
        button_secondary_hover: secondary.darken(0.03),
        button_success: success,
        button_success_active: success.darken(0.10),
        button_success_foreground: success_foreground,
        button_success_hover: success.darken(0.05),
        button_warning: warning,
        button_warning_active: warning.darken(0.10),
        button_warning_foreground: warning_foreground,
        button_warning_hover: warning.darken(0.05),
        success,
        success_active: success.darken(0.10),
        success_foreground,
        success_hover: success.darken(0.05),
        group_box: muted,
        group_box_foreground: foreground,
        caret: primary,
        chart_1: hex_color("#3D7CA8"),
        chart_2: hex_color("#5AA9E6"),
        chart_3: hex_color("#7FBFEE"),
        chart_4: hex_color("#A8D2F2"),
        chart_5: hex_color("#CFE5F8"),
        chart_bullish: success,
        chart_bearish: danger,
        chart_grid: border,
        danger,
        danger_active: danger.darken(0.10),
        danger_foreground,
        danger_hover: danger.darken(0.05),
        description_list_label: muted,
        description_list_label_foreground: muted_foreground,
        drag_border: accent,
        drop_target: accent.opacity(0.20),
        foreground,
        info,
        info_active: info.darken(0.10),
        info_foreground,
        info_hover: info.darken(0.05),
        input: border_strong,
        link: primary,
        link_active: primary_active,
        link_hover: primary_hover,
        list: panel,
        list_active,
        list_active_border: primary.with_alpha(0.60),
        list_even: panel,
        list_head: muted,
        list_hover,
        muted,
        muted_foreground,
        popover: panel,
        popover_foreground: foreground,
        primary,
        primary_active,
        primary_foreground,
        primary_hover,
        progress_bar,
        ring,
        scrollbar: surface,
        scrollbar_thumb: muted_foreground.opacity(0.50),
        scrollbar_thumb_hover: muted_foreground.opacity(0.80),
        secondary,
        secondary_active: secondary.darken(0.06),
        secondary_foreground,
        secondary_hover: secondary.darken(0.03),
        selection,
        sidebar,
        sidebar_accent: list_active,
        sidebar_accent_foreground: primary,
        sidebar_border: border,
        sidebar_foreground: foreground,
        sidebar_primary: primary,
        sidebar_primary_foreground: primary_foreground,
        skeleton: muted,
        slider_bar: primary,
        slider_thumb: primary_foreground,
        switch: muted,
        switch_thumb: panel,
        tab: surface,
        tab_active: panel,
        tab_active_foreground: foreground,
        tab_bar: panel,
        tab_bar_segmented: muted,
        tab_foreground: muted_foreground,
        table: panel,
        table_active: list_active,
        table_active_border: primary.with_alpha(0.60),
        table_even: panel,
        table_head: muted,
        table_head_foreground: foreground,
        table_foot: muted,
        table_foot_foreground: foreground,
        table_hover: list_hover,
        table_row_border: border,
        title_bar: topbar,
        title_bar_border: border,
        status_bar: topbar,
        status_bar_border: border,
        warning,
        warning_active: warning.darken(0.10),
        warning_foreground,
        warning_hover: warning.darken(0.05),
        overlay: foreground.opacity(0.55),
        window_border: border,
        red: danger,
        red_light: danger.with_alpha(0.20),
        green: success,
        green_light: success.with_alpha(0.20),
        blue: accent,
        blue_light: accent.with_alpha(0.20),
        yellow: warning,
        yellow_light: warning.with_alpha(0.20),
        magenta: hex_color("#F08FB0"),
        magenta_light: hex_color("#F8C8D8"),
        cyan: accent,
        cyan_light: accent.with_alpha(0.20),
    }
}

// MARK: MotionTokens

fn motion_tokens() -> MotionTokens {
    MotionTokens {
        duration_instant: Duration::ZERO,
        duration_fast: Duration::from_millis(140),
        duration_normal: Duration::from_millis(180),
        duration_slow: Duration::from_millis(260),
        easing_enter: Easing::cubic_bezier(0.16, 1.0, 0.3, 1.0)
            .expect("static enter curve is valid"),
        easing_exit: Easing::cubic_bezier(0.4, 0.0, 1.0, 1.0).expect("static exit curve is valid"),
        easing_move: Easing::cubic_bezier(0.2, 0.0, 0.0, 1.0).expect("static move curve is valid"),
        spring_control: Spring::new(Duration::from_millis(220)).with_damping(0.85),
        spring_move: Spring::new(Duration::from_millis(320))
            .with_damping(0.85)
            .with_epsilon(0.1),
        distance_short: crate::gpui::rems(0.25),
        distance_medium: crate::gpui::rems(0.5),
    }
}

// 单元测试：确保调色板构造无 panic + Phase 3 Gate 3 回归测试。
#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpui::TestAppContext;
    use crate::gpui::component::{Theme, ThemeMode};

    #[test]
    fn palettes_construct_without_panic() {
        let _ = light_palette();
        let _ = dark_palette();
    }

    /// DESIGN.md §2.1：关键 token 的 hex 与表一致。
    ///
    /// 容忍度：≤2/通道——hex 是设计表契约；HSL 反算引入 ≤1 通道浮点漂移。
    #[test]
    fn key_token_hex_matches_design_table() {
        let lp = light_palette();
        assert_eq_hex("surface", &hsla_to_hex(lp.background), "#FBF6EC");
        assert_eq_hex("primary", &hsla_to_hex(lp.primary), "#F08A36");
        assert_eq_hex("sidebar", &hsla_to_hex(lp.sidebar), "#F5EBD9");
        assert_eq_hex("panel", &hsla_to_hex(lp.popover), "#FFFCF5");
        assert_eq_hex("border", &hsla_to_hex(lp.border), "#EADFC8");
        assert_eq_hex("foreground", &hsla_to_hex(lp.foreground), "#2A2620");
        assert_eq_hex(
            "muted_foreground",
            &hsla_to_hex(lp.muted_foreground),
            "#7A6F60",
        );
        // 加深的 fill（保证白字 AA ≥4.5）
        assert_eq_hex("accent (darkened)", &hsla_to_hex(lp.accent), "#3478AC");
        assert_eq_hex("success (darkened)", &hsla_to_hex(lp.success), "#3D8246");
        assert_eq_hex("warning (darkened)", &hsla_to_hex(lp.warning), "#8F6612");
        assert_eq_hex("danger (darkened)", &hsla_to_hex(lp.danger), "#9C2929");
        assert_eq_hex("muted", &hsla_to_hex(lp.muted), "#F1E8D5");
        assert_eq!(
            hsla_to_hex_full(lp.selection),
            "#F08A363D",
            "light selection (alpha)"
        );
        assert_eq!(
            hsla_to_hex_full(lp.list_active),
            "#F08A3633",
            "light list_active (alpha)"
        );
        assert_eq_hex("border_strong", &hsla_to_hex(lp.input), "#D9C8A3");

        let dp = dark_palette();
        assert_eq_hex("dark surface", &hsla_to_hex(dp.background), "#1B1815");
        assert_eq_hex("dark primary", &hsla_to_hex(dp.primary), "#FFA55C");
        assert_eq_hex("dark sidebar", &hsla_to_hex(dp.sidebar), "#1F1B17");
        assert_eq_hex("dark panel", &hsla_to_hex(dp.popover), "#26221E");
        assert_eq_hex("dark border", &hsla_to_hex(dp.border), "#3A342C");
        assert_eq_hex("dark foreground", &hsla_to_hex(dp.foreground), "#F2E9D6");
        assert_eq_hex(
            "dark muted_foreground",
            &hsla_to_hex(dp.muted_foreground),
            "#A89A82",
        );
        assert_eq_hex("dark accent", &hsla_to_hex(dp.accent), "#7FBFEE");
        assert_eq_hex("dark success", &hsla_to_hex(dp.success), "#7CCE89");
        assert_eq_hex("dark warning", &hsla_to_hex(dp.warning), "#FFC664");
        assert_eq_hex("dark danger", &hsla_to_hex(dp.danger), "#F08B8B");
        assert_eq!(
            hsla_to_hex_full(dp.selection),
            "#5A3D1A4D",
            "dark selection (alpha)"
        );
        assert_eq!(
            hsla_to_hex_full(dp.list_active),
            "#5A3D1A33",
            "dark list_active (alpha)"
        );
    }

    fn assert_eq_hex(label: &str, got: &str, expected: &str) {
        let got_rgb = parse_hex(got);
        let exp_rgb = parse_hex(expected);
        // 容忍度 ≤2 / 通道——hex 是契约；HSL 反算引入 ≤1 通道浮点漂移。
        const TOL: u8 = 2;
        assert!(
            (got_rgb.0.abs_diff(exp_rgb.0) <= TOL)
                && (got_rgb.1.abs_diff(exp_rgb.1) <= TOL)
                && (got_rgb.2.abs_diff(exp_rgb.2) <= TOL),
            "{label}: got {got}, expected {expected} (±{TOL} per channel); got rgb={got_rgb:?} vs expected {exp_rgb:?}"
        );
    }

    fn parse_hex(s: &str) -> (u8, u8, u8) {
        let s = s.trim_start_matches('#');
        let r = u8::from_str_radix(&s[0..2], 16).unwrap();
        let g = u8::from_str_radix(&s[2..4], 16).unwrap();
        let b = u8::from_str_radix(&s[4..6], 16).unwrap();
        (r, g, b)
    }

    #[test]
    fn motion_tokens_within_design_range() {
        let m = motion_tokens();
        assert!(m.duration_fast.as_millis() < m.duration_normal.as_millis());
        assert!(m.duration_normal.as_millis() < m.duration_slow.as_millis());
    }

    /// `hsla_to_hex` 应正确把已知颜色转 `#RRGGBB`，无 alpha 通道。
    #[test]
    fn hsla_to_hex_known_colors() {
        // gpui::Hsla.h 是 [0, 1) 归一化角度。我们用 `hsl_deg` 走度数路径，
        // 验证 hsl_deg → hsl_to_rgb → hex 链路对常见色正确。
        use crate::ui::palette::hsl_deg;

        // h = 0° = red #FF0000
        assert_eq!(hsla_to_hex(hsl_deg(0.0, 1.0, 0.5)), "#FF0000");
        // h = 120° = green #00FF00
        assert_eq!(hsla_to_hex(hsl_deg(120.0, 1.0, 0.5)), "#00FF00");
        // h = 240° = blue #0000FF
        assert_eq!(hsla_to_hex(hsl_deg(240.0, 1.0, 0.5)), "#0000FF");
        // h = 60° = yellow #FFFF00
        assert_eq!(hsla_to_hex(hsl_deg(60.0, 1.0, 0.5)), "#FFFF00");

        // 灰阶
        assert_eq!(hsla_to_hex(hsl_deg(0.0, 0.0, 1.0)), "#FFFFFF");
        assert_eq!(hsla_to_hex(hsl_deg(0.0, 0.0, 0.0)), "#000000");
        assert_eq!(hsla_to_hex(hsl_deg(0.0, 0.0, 0.5)), "#808080");
    }

    /// `hex_color` 往返：常用 hex 与还原的 Hsla→hex 一致。
    #[test]
    fn hex_color_roundtrip() {
        let cases = [
            "#FF0000", "#00FF00", "#0000FF", "#FFFF00", "#FFFFFF", "#000000",
            "#5AA9E6", // accent light
            "#FBF6EC", // surface light
            "#1B1815", // background dark
            "#FFA55C", // primary dark
        ];
        for hex in cases {
            let h = hex_color(hex);
            assert_eq!(
                hsla_to_hex(h),
                hex,
                "roundtrip failed for {hex}: got hsla = {:?}, hex = {}",
                h,
                hsla_to_hex(h),
            );
        }
    }

    /// `hex_color` 支持 8 位带 alpha。
    #[test]
    fn hex_color_with_alpha() {
        let h = hex_color("#F08A3680");
        // alpha 0x80 = 128 ≈ 0.502
        assert!((h.a - 0.502).abs() < 0.01, "alpha = {}", h.a);
        assert_eq!(hsla_to_hex_full(h), "#F08A3680");
    }

    /// WCAG AA 对比度断言：所有填色上的前景文字均 ≥4.5。
    /// 来源：DESIGN.md §5.1 + §2.1/§2.2 token 表。
    #[test]
    fn wcag_aa_contrast_on_filled_buttons() {
        fn check(label: &str, fg_hex: &str, bg_hex: &str, min: f32) {
            let cr = contrast_ratio(hex_color(fg_hex), hex_color(bg_hex));
            assert!(
                cr >= min,
                "{label}: fg={fg_hex} on bg={bg_hex} → cr={cr:.2} (期望 ≥{min})"
            );
        }
        // 浅色模式：白字仅用在已加深的 fill 上；primary_fg ink 用在饱和主橙上
        check("light primary_ink on primary", "#1F1612", "#F08A36", 4.5);
        check("light accent_white on accent", "#FFFFFF", "#3478AC", 4.5);
        check("light success_white on success", "#FFFFFF", "#3D8246", 4.5);
        check("light warning_white on warning", "#FFFFFF", "#8F6612", 4.5);
        check("light danger_white on danger", "#FFFFFF", "#9C2929", 4.5);
        check("light foreground on surface", "#2A2620", "#FBF6EC", 7.0);
        check(
            "light muted_foreground on surface",
            "#7A6F60",
            "#FBF6EC",
            4.5,
        );
        check("light secondary_fg on secondary", "#5A4422", "#FFF1DD", 4.5);
        // 暗色模式：深棕字 + 浅橙底 = AAA；其余沿用旧策略（暗 fg + 浅色 fill）
        check("dark primary_fg on primary", "#1B1815", "#FFA55C", 4.5);
        check("dark foreground on background", "#F2E9D6", "#1B1815", 7.0);
        check(
            "dark muted_foreground on background",
            "#A89A82",
            "#1B1815",
            4.5,
        );
        check("dark success_fg on success", "#0E2410", "#7CCE89", 4.5);
        check("dark danger_fg on danger", "#2E0F0F", "#F08B8B", 4.5);
        check("dark warning_fg on warning", "#3D2A0F", "#FFC664", 4.5);
        check("dark accent_fg on accent", "#0F1F2E", "#7FBFEE", 4.5);
    }

    /// `theme_config_colors_from` 的键集合必须与 vendor `ThemeConfigColors` 完全一致。
    ///
    /// 这能抓住三类缺陷：① 键名拼错（如 `progress_bar.background` vs
    /// `progress.bar.background`）；② 漏字段（如 `chart.1..5`）；③ 公共字段没赋值
    /// （序列化为 null，`apply_config` 会回退到 gpui 默认色）。
    #[test]
    fn theme_config_keys_match_vendor_schema() {
        let default = serde_json::to_value(ThemeConfigColors::default()).unwrap();
        let expected: std::collections::BTreeSet<String> = default
            .as_object()
            .expect("ThemeConfigColors 应序列化为 object")
            .keys()
            .cloned()
            .collect();

        for (mode_name, palette) in [("light", light_palette()), ("dark", dark_palette())] {
            let cfg = theme_config_colors_from(&palette);
            let value = serde_json::to_value(&cfg).unwrap();
            let object = value.as_object().expect("应序列化为 object");
            let actual: std::collections::BTreeSet<String> = object.keys().cloned().collect();

            assert_eq!(
                actual, expected,
                "{mode_name}: ThemeConfigColors 键集合与 vendor 不一致"
            );

            for (key, value) in object {
                if key.starts_with("base.") {
                    // 12 个私有基础色保持 None：它们只是缺失公共字段的 fallback。
                    assert!(value.is_null(), "{mode_name}: {key} 不应被设置");
                } else {
                    assert!(
                        !value.is_null(),
                        "{mode_name}: {key} 未赋值，直接 Theme::change 会回退到 gpui 默认色"
                    );
                }
            }

            // 关键 alpha 键必须是 #RRGGBBAA，否则对话框遮罩/滚动条会变不透明。
            let expect_alpha = |key: &str, alpha: u8| {
                let s = object[key]
                    .as_str()
                    .unwrap_or_else(|| panic!("{mode_name}: {key} 不是字符串"));
                assert_eq!(
                    s.len(),
                    9,
                    "{mode_name}: {key} 应带 alpha（#RRGGBBAA），got {s}"
                );
                let got = u8::from_str_radix(&s[7..9], 16).unwrap();
                assert!(
                    got.abs_diff(alpha) <= 1,
                    "{mode_name}: {key} alpha = {got:#04X}，期望 {alpha:#04X}（{s}）"
                );
            };
            let (overlay_alpha, border_alpha) = if mode_name == "light" {
                (0x52, 0x80)
            } else {
                (0x8C, 0x99)
            };
            expect_alpha("overlay", overlay_alpha);
            expect_alpha("scrollbar.thumb.background", 0x80);
            expect_alpha("drop_target.background", 0x33);
            expect_alpha("list.active.border", border_alpha);
            expect_alpha("table.active.border", border_alpha);
            expect_alpha("list.active.background", 0x33);
            expect_alpha(
                "selection.background",
                if mode_name == "light" { 0x3D } else { 0x4D },
            );
        }
    }

    /// `theme_config_colors_from` 应能正确序列化所有公共字段；
    /// 私有 base color 字段被 serde 默认填充。
    #[test]
    fn theme_config_colors_roundtrip() {
        for (mode_name, palette) in [("light", light_palette()), ("dark", dark_palette())] {
            let tcc = theme_config_colors_from(&palette);
            // 抽查：primary / sidebar / background 应正确回填
            assert!(tcc.primary.is_some(), "{mode_name} primary 必须被设置");
            assert!(tcc.sidebar.is_some(), "{mode_name} sidebar 必须被设置");
            assert!(
                tcc.background.is_some(),
                "{mode_name} background 必须被设置"
            );
            assert!(
                tcc.muted_foreground.is_some(),
                "{mode_name} muted_foreground 必须被设置"
            );
        }
    }

    // --- Gate 3 回归：验证直接 `Theme::change` 不会丢失 CoursePlan 调色板。 ---

    fn fresh_cx() -> TestAppContext {
        TestAppContext::single()
    }

    /// 颜色近似比较：RGB ≤2/通道，alpha ≤1/255。
    fn assert_color_close(label: &str, before: Hsla, after: Hsla) {
        let b = hsla_to_hex_full(before);
        let a = hsla_to_hex_full(after);
        let (br, bg, bb) = parse_hex(&b);
        let (ar, ag, ab) = parse_hex(&a);
        assert!(
            br.abs_diff(ar) <= 2 && bg.abs_diff(ag) <= 2 && bb.abs_diff(ab) <= 2,
            "{label}: {b} → {a}（RGB 偏差 >2）"
        );
        assert!(
            (before.a - after.a).abs() <= 1.5 / 255.0,
            "{label}: alpha {} → {}（偏差 >1/255）",
            before.a,
            after.a
        );
    }

    macro_rules! assert_color_fields {
        ($before:expr, $after:expr, $($field:ident),+ $(,)?) => {
            $(
                assert_color_close(concat!("colors.", stringify!($field)), $before.$field, $after.$field);
            )+
        };
    }

    macro_rules! assert_token_fields {
        ($before:expr, $after:expr, $($field:ident),+ $(,)?) => {
            $(
                assert_color_close(concat!("tokens.", stringify!($field)), $before.$field.color, $after.$field.color);
            )+
        };
    }

    /// 直接调用 `Theme::change`（不经过 `apply_for_mode`）后，所有 CoursePlan 语义色
    /// 必须与刚安装时一致——包括 `overlay.a`、`progress_bar`、`chart_*`、`tokens.*`。
    ///
    /// 12 个 vendor 基础色（`red..cyan_light`）不可通过 `ThemeConfigColors` 设置，
    /// 只作为缺失公共字段的 fallback，因此不参与比较。
    #[test]
    fn direct_theme_change_preserves_all_semantic_fields() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let cx = fresh_cx();
            cx.update(|cx| {
                crate::gpui::component::theme::init(cx);
                force_init_for_test(cx);
                crate::gpui::component::Theme::change(mode, None, cx);

                let theme = Theme::global(cx);
                let expected = if mode.is_dark() {
                    dark_palette()
                } else {
                    light_palette()
                };

                assert_color_fields!(
                    expected,
                    theme.colors,
                    accent,
                    accent_foreground,
                    accordion,
                    background,
                    border,
                    button,
                    button_active,
                    button_foreground,
                    button_hover,
                    button_danger,
                    button_danger_active,
                    button_danger_foreground,
                    button_danger_hover,
                    button_info,
                    button_info_active,
                    button_info_foreground,
                    button_info_hover,
                    button_primary,
                    button_primary_active,
                    button_primary_foreground,
                    button_primary_hover,
                    button_secondary,
                    button_secondary_active,
                    button_secondary_foreground,
                    button_secondary_hover,
                    button_success,
                    button_success_active,
                    button_success_foreground,
                    button_success_hover,
                    button_warning,
                    button_warning_active,
                    button_warning_foreground,
                    button_warning_hover,
                    group_box,
                    group_box_foreground,
                    caret,
                    chart_1,
                    chart_2,
                    chart_3,
                    chart_4,
                    chart_5,
                    chart_bullish,
                    chart_bearish,
                    chart_grid,
                    danger,
                    danger_active,
                    danger_foreground,
                    danger_hover,
                    description_list_label,
                    description_list_label_foreground,
                    drag_border,
                    drop_target,
                    foreground,
                    info,
                    info_active,
                    info_foreground,
                    info_hover,
                    input,
                    link,
                    link_active,
                    link_hover,
                    list,
                    list_active,
                    list_active_border,
                    list_even,
                    list_head,
                    list_hover,
                    muted,
                    muted_foreground,
                    popover,
                    popover_foreground,
                    primary,
                    primary_active,
                    primary_foreground,
                    primary_hover,
                    progress_bar,
                    ring,
                    scrollbar,
                    scrollbar_thumb,
                    scrollbar_thumb_hover,
                    secondary,
                    secondary_active,
                    secondary_foreground,
                    secondary_hover,
                    selection,
                    sidebar,
                    sidebar_accent,
                    sidebar_accent_foreground,
                    sidebar_border,
                    sidebar_foreground,
                    sidebar_primary,
                    sidebar_primary_foreground,
                    skeleton,
                    slider_bar,
                    slider_thumb,
                    success,
                    success_foreground,
                    success_hover,
                    success_active,
                    switch,
                    switch_thumb,
                    tab,
                    tab_active,
                    tab_active_foreground,
                    tab_bar,
                    tab_bar_segmented,
                    tab_foreground,
                    table,
                    table_active,
                    table_active_border,
                    table_even,
                    table_head,
                    table_head_foreground,
                    table_foot,
                    table_foot_foreground,
                    table_hover,
                    table_row_border,
                    title_bar,
                    title_bar_border,
                    status_bar,
                    status_bar_border,
                    warning,
                    warning_active,
                    warning_hover,
                    warning_foreground,
                    overlay,
                    window_border,
                );

                // 重点回归：overlay alpha 不能丢（对话框遮罩）。
                let expected_alpha = if mode.is_dark() { 0.55 } else { 0.32 };
                assert!(
                    (theme.overlay.a - expected_alpha).abs() < 0.01,
                    "overlay.a = {}（期望 {expected_alpha}）",
                    theme.overlay.a
                );
                assert_color_close("progress_bar", expected.progress_bar, theme.progress_bar);
                assert_color_close("chart_2", expected.chart_2, theme.chart_2);
                assert_color_close("chart_5", expected.chart_5, theme.chart_5);

                // tokens.* 是组件实际读取的解析结果；全部对比 color。
                let expected_tokens = crate::gpui::component::ThemeTokens::from(expected);
                assert_token_fields!(
                    expected_tokens,
                    theme.tokens,
                    accent,
                    accent_foreground,
                    accordion,
                    background,
                    border,
                    button,
                    button_active,
                    button_foreground,
                    button_hover,
                    button_danger,
                    button_danger_active,
                    button_danger_foreground,
                    button_danger_hover,
                    button_info,
                    button_info_active,
                    button_info_foreground,
                    button_info_hover,
                    button_primary,
                    button_primary_active,
                    button_primary_foreground,
                    button_primary_hover,
                    button_secondary,
                    button_secondary_active,
                    button_secondary_foreground,
                    button_secondary_hover,
                    button_success,
                    button_success_active,
                    button_success_foreground,
                    button_success_hover,
                    button_warning,
                    button_warning_active,
                    button_warning_foreground,
                    button_warning_hover,
                    group_box,
                    group_box_foreground,
                    caret,
                    chart_1,
                    chart_2,
                    chart_3,
                    chart_4,
                    chart_5,
                    chart_bullish,
                    chart_bearish,
                    chart_grid,
                    danger,
                    danger_active,
                    danger_foreground,
                    danger_hover,
                    description_list_label,
                    description_list_label_foreground,
                    drag_border,
                    drop_target,
                    foreground,
                    info,
                    info_active,
                    info_foreground,
                    info_hover,
                    input,
                    link,
                    link_active,
                    link_hover,
                    list,
                    list_active,
                    list_active_border,
                    list_even,
                    list_head,
                    list_hover,
                    muted,
                    muted_foreground,
                    popover,
                    popover_foreground,
                    primary,
                    primary_active,
                    primary_foreground,
                    primary_hover,
                    progress_bar,
                    ring,
                    scrollbar,
                    scrollbar_thumb,
                    scrollbar_thumb_hover,
                    secondary,
                    secondary_active,
                    secondary_foreground,
                    secondary_hover,
                    selection,
                    sidebar,
                    sidebar_accent,
                    sidebar_accent_foreground,
                    sidebar_border,
                    sidebar_foreground,
                    sidebar_primary,
                    sidebar_primary_foreground,
                    skeleton,
                    slider_bar,
                    slider_thumb,
                    success,
                    success_foreground,
                    success_hover,
                    success_active,
                    switch,
                    switch_thumb,
                    tab,
                    tab_active,
                    tab_active_foreground,
                    tab_bar,
                    tab_bar_segmented,
                    tab_foreground,
                    table,
                    table_active,
                    table_active_border,
                    table_even,
                    table_head,
                    table_head_foreground,
                    table_foot,
                    table_foot_foreground,
                    table_hover,
                    table_row_border,
                    title_bar,
                    title_bar_border,
                    status_bar,
                    status_bar_border,
                    warning,
                    warning_active,
                    warning_hover,
                    warning_foreground,
                    overlay,
                    window_border,
                );
            });
        }
    }

    #[test]
    fn direct_theme_change_preserves_light_palette() {
        let cx = fresh_cx();
        cx.update(|cx| {
            // 1) 先安装 gpui-component 主题系统（创建 Theme 全局），
            //    这会注册 ThemeRegistry 默认 + 调一次 Theme::change(Light)。
            crate::gpui::component::theme::init(cx);

            // 2) 再装载 CoursePlan 主题——这一步装 ThemeConfig + 写 colors。
            force_init_for_test(cx);
            assert_eq!(Theme::global(cx).mode, ThemeMode::Light);

            // 3) 直接 Theme::change(Light) —— 不能让任何"reload"路径 clobber 我们
            crate::gpui::component::Theme::change(ThemeMode::Light, None, cx);

            let t = Theme::global(cx);
            assert_eq!(t.mode, ThemeMode::Light);
            // 验证关键字段在经过 hex round-trip 后仍与调色板一致。
            //
            // 注意：`h` 在 try_parse_color 的 hex→RGB→HSL 转换中会因浮点路径分支而漂移
            // （例如 `hsl(1.0)` 视为红，`(244, 49, 49)` 反算回 `h=0`），
            // 但视觉颜色一致。`l` 和 `s` 通过 (max+min)/2 与 delta/(2*l) 反算，保留稳定。
            // 我们只比对 l/s 与 l-s 派生量。
            //
            // 容忍度：低饱和色（s<0.25）反算 s 可能漂 0.05；高饱和色 s 漂 <0.02。
            let bg = t.background;
            let expected = light_palette().background;
            assert!(
                (bg.l - expected.l).abs() < 0.02,
                "background.l = {} (期望 ~{})",
                bg.l,
                expected.l
            );
            assert!(
                (bg.s - expected.s).abs() < 0.06,
                "background.s = {} (期望 ~{})",
                bg.s,
                expected.s
            );

            // primary：用 L、S 验证；HSL→RGB→HSL 对纯色 / 饱和色 会让 h 在某些分支归 0。
            let primary = t.primary;
            let expected_p = light_palette().primary;
            assert!(
                (primary.l - expected_p.l).abs() < 0.02,
                "primary.l = {} (期望 ~{}): {:?}",
                primary.l,
                expected_p.l,
                primary
            );
            assert!(
                (primary.s - expected_p.s).abs() < 0.02,
                "primary.s = {} (期望 ~{}): {:?}",
                primary.s,
                expected_p.s,
                primary
            );

            // sidebar
            let sidebar = t.sidebar;
            let expected_s = light_palette().sidebar;
            assert!(
                (sidebar.l - expected_s.l).abs() < 0.02,
                "sidebar.l = {} (期望 ~{})",
                sidebar.l,
                expected_s.l
            );

            // muted_foreground
            let mf = t.muted_foreground;
            let expected_mf = light_palette().muted_foreground;
            assert!(
                (mf.l - expected_mf.l).abs() < 0.02,
                "muted_foreground.l = {} (期望 ~{})",
                mf.l,
                expected_mf.l
            );

            // radius 写入正确
            assert_eq!(t.radius, gpui::px(RADIUS));
        });
    }

    #[test]
    fn direct_theme_change_preserves_dark_palette() {
        let cx = fresh_cx();
        cx.update(|cx| {
            crate::gpui::component::theme::init(cx);
            force_init_for_test(cx);

            // 直接切到 Dark，不调 apply_for_mode——只有 init 已装好的 ThemeConfig 起作用
            crate::gpui::component::Theme::change(ThemeMode::Dark, None, cx);

            let t = Theme::global(cx);
            assert_eq!(t.mode, ThemeMode::Dark);
            // dark primary 通过 L/S 验证
            let expected_p = dark_palette().primary;
            assert!(
                (t.primary.l - expected_p.l).abs() < 0.02
                    && (t.primary.s - expected_p.s).abs() < 0.02,
                "dark primary L/S 不对: {:?} vs {:?}",
                t.primary,
                expected_p
            );
            // dark background
            let expected_bg = dark_palette().background;
            assert!(
                (t.background.l - expected_bg.l).abs() < 0.02,
                "dark background.l 不对: {} vs {}",
                t.background.l,
                expected_bg.l
            );
        });
    }

    #[test]
    fn toggle_mode_round_trip() {
        let cx = fresh_cx();
        cx.update(|cx| {
            crate::gpui::component::theme::init(cx);
            force_init_for_test(cx);
            assert_eq!(Theme::global(cx).mode, ThemeMode::Light);

            toggle_mode(None, cx);
            assert_eq!(Theme::global(cx).mode, ThemeMode::Dark);
            // toggle_mode 后调色板仍是 dark（apply_for_mode 写回精确 Hsla）
            let p_dark = Theme::global(cx).primary;
            let expected = dark_palette().primary;
            assert!(
                (p_dark.l - expected.l).abs() < 0.02 && (p_dark.s - expected.s).abs() < 0.02,
                "toggle dark primary 不匹配: {:?} vs {:?}",
                p_dark,
                expected
            );

            toggle_mode(None, cx);
            assert_eq!(Theme::global(cx).mode, ThemeMode::Light);
            let p_light = Theme::global(cx).primary;
            let expected = light_palette().primary;
            assert!(
                (p_light.l - expected.l).abs() < 0.02 && (p_light.s - expected.s).abs() < 0.02,
                "toggle light primary 不匹配: {:?} vs {:?}",
                p_light,
                expected
            );
        });
    }

    /// init 后不应回退到 gpui-component 默认色。
    #[test]
    fn init_does_not_fall_back_to_gpui_component_defaults() {
        let cx = fresh_cx();
        cx.update(|cx| {
            crate::gpui::component::theme::init(cx);
            force_init_for_test(cx);
            let t = Theme::global(cx);
            // 我们 light_palette().background.l ≈ 0.965
            // gpui-component 默认 ThemeColor::light().background.l ≈ 1.0
            assert!(
                t.background.l < 0.99,
                "background.l = {} 接近白色（gpui 默认），应低于 0.99 才算 CoursePlan 调色板",
                t.background.l
            );
            // radius 必须 = RADIUS (10)，不是 gpui-component 默认 6
            assert_eq!(t.radius, gpui::px(RADIUS));
        });
    }
}
