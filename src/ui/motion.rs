//! 动效原语（详见 `docs/DESIGN.md` §2.10、§3.17-3.18）。
//!
//! 设计原则：动效是奖励，不是装饰。`with_animation` 会自动遵守
//! `App::reduce_motion()`（reduce 时直接落到 oneshot 的终态）。

use crate::gpui::Animation;
use crate::gpui::base::Easing;
use crate::gpui::component::{MotionTokens, Theme};
use std::time::Duration;

/// 全局页面淡入曲线：透明度 0→1 + translateY(4px → 0)。
pub fn page_enter_easing() -> Easing {
    Easing::cubic_bezier(0.16, 1.0, 0.3, 1.0).expect("static enter curve is valid")
}

/// 全局页面退出曲线。
pub fn page_exit_easing() -> Easing {
    Easing::cubic_bezier(0.4, 0.0, 1.0, 1.0).expect("static exit curve is valid")
}

/// 全局平面位移曲线。
pub fn move_easing() -> Easing {
    Easing::cubic_bezier(0.2, 0.0, 0.0, 1.0).expect("static move curve is valid")
}

/// 页面切换动画时长（§3.18：260ms；reduce-motion 时立刻完成）。
pub fn page_enter_duration(reduce_motion: bool) -> Duration {
    if reduce_motion {
        Duration::ZERO
    } else {
        Duration::from_millis(260)
    }
}

/// 页面进入动画（§3.18）：260ms + `easing_enter`。
///
/// 用法（app.rs 已用）：
/// ```ignore
/// content.with_animation(
///     ElementId::from(format!("page-enter:{}", page.label())),
///     motion::page_enter_animation(),
///     |el, delta| el.opacity(delta).mt(px(4. * (1.0 - delta))),
/// )
/// ```
pub fn page_enter_animation() -> Animation {
    let easing = page_enter_easing();
    Animation::new(page_enter_duration(false)).with_easing(move |delta| easing.sample(delta))
}

/// 打卡勾选弹跳（§3.17：一次性的「弹一下」）。
///
/// 这是视图可用的弹跳 API：`gpui_base::motion::spring(id, target, policy, window, cx)`
/// 需要 `&mut Window`，而视图的 `render(app, cx)` 拿不到；本 gpui 分支也没有
/// CSS transform（`scale()`/`rotate()`），因此弹跳以等价的一次性 `Animation`
/// （`duration_slow` + ease-out-back 过冲）表达，由调用方把脉冲折进自己的
/// 几何 / 颜色（见 `views::checkins::check_burst`：圆环直径 + 光晕）。
/// reduce-motion 由 `with_animation` 自动落到终态。
pub fn bounce_animation(theme: &Theme) -> Animation {
    let duration = tokens(theme).duration_slow;
    let curve = Easing::cubic_bezier(0.34, 1.56, 0.64, 1.0).expect("ease-out-back 控制点合法");
    Animation::new(duration).with_easing(move |t| curve.sample(t))
}

/// 把过冲后的曲线采样值折成「0 → 1 → 0」的脉冲（终态恒为 0）。
pub fn bounce_pulse(delta: f32) -> f32 {
    (1.0 - (2.0 * delta - 1.0).abs()).clamp(0.0, 1.0)
}

/// 从 Theme.motion 派生：把所有令牌整合为只读快照。
pub fn tokens(theme: &Theme) -> &MotionTokens {
    theme.motion_tokens()
}
