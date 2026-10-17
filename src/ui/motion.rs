//! 动效原语（详见 `docs/DESIGN.md` §2.10、§3.17-3.18）。
//!
//! 设计原则：动效是奖励，不是装饰。所有 spring/scale/rotate 必须遵守
//! `App::reduce_motion()`——开启时退化为 0ms 瞬时切换。

use crate::gpui::base::Easing;
use crate::gpui::component::{MotionTokens, Theme};
use std::time::Duration;

/// 全局页面淡入曲线：透明度 0→1 + translateY(4px → 0)，180ms ease-out。
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

/// 页面切换动画时长（亮暗模式一致；reduce-motion 时立刻完成）。
pub fn page_enter_duration(reduce_motion: bool) -> Duration {
    if reduce_motion {
        Duration::ZERO
    } else {
        Duration::from_millis(260)
    }
}

/// 打卡勾选弹跳 spring（reduce-motion 时退化为 0ms）。
pub fn bounce_spring(reduce_motion: bool) -> crate::gpui::base::Spring {
    if reduce_motion {
        crate::gpui::base::Spring::new(Duration::ZERO)
    } else {
        crate::gpui::base::Spring::new(Duration::from_millis(220)).with_damping(0.65)
    }
}

/// 从 Theme.motion 派生：把所有令牌整合为只读快照。
pub fn tokens(theme: &Theme) -> &MotionTokens {
    theme.motion_tokens()
}
