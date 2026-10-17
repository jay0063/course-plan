//! 设置（DESIGN §4.8）。
//!
//! Phase 4 V3：单列卡片堆叠——学生信息（输入 + `DatePicker` 学期日期）、
//! 提醒与启动（两个 Switch）、外观主题（`tab_chips` 浅色 / 深色）、同步
//! （**仅样式**，行为与代码路径原样保留）、数据存储（`kv_row` 计数 + 数据目录
//! 链接 + 载入示例 / 清空全部，二次确认）。
//!
//! 页面名与副标题归顶栏（§3.1.4）——内容区首块直接是「学生信息」卡片。

use crate::prelude::*;
use chrono::NaiveDate;
use gpui::assets::IconName;
use gpui::component::ThemeMode;
use gpui::component::WindowExt;
use gpui::component::button::{Button, ButtonVariant};
use gpui::component::date_picker::{DatePicker, DatePickerState, DateTime};
use gpui::component::input::{Input, InputState};
use gpui::component::notification::Notification;
use gpui::component::scroll::ScrollableElement;
use gpui::component::switch::Switch;
use gpui::*;

use crate::app::App;
use crate::model::Profile;
use crate::sync::{Status, SyncCommand};
use crate::ui::components::button as btn;
use crate::ui::components::card::{CardVariant, card_with_theme};
use crate::ui::components::kv_row::kv_row;
use crate::ui::components::tab_chips::tab_chips;
use crate::ui::form::field_row;
use crate::ui::tokens::{INPUT_H, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XS};
use crate::ui::widgets::eid;

/// 学生资料编辑区。作为独立视图，方便读取输入值并写回 App。
pub struct ProfileInputs {
    app: Entity<App>,
    name: Entity<InputState>,
    grade: Entity<InputState>,
    school: Entity<InputState>,
    term_start: Entity<DatePickerState>,
    term_end: Entity<DatePickerState>,
}

impl ProfileInputs {
    pub fn new(
        profile: &Profile,
        app: Entity<App>,
        window: &mut Window,
        cx: &mut Context<App>,
    ) -> Entity<Self> {
        let n = profile.student.clone();
        let g = profile.grade.clone();
        let s = profile.school.clone();
        let ts = parse_ymd(&profile.term_start);
        let te = parse_ymd(&profile.term_end);

        let term_start = cx.new(|cx| DatePickerState::new(window, cx).date_format("%Y-%m-%d"));
        let term_end = cx.new(|cx| DatePickerState::new(window, cx).date_format("%Y-%m-%d"));
        if let Some(date) = ts {
            term_start.update(cx, |state, cx| state.set_date(date, window, cx));
        }
        if let Some(date) = te {
            term_end.update(cx, |state, cx| state.set_date(date, window, cx));
        }

        cx.new(|cx| Self {
            app,
            name: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("学生姓名")
                    .default_value(n)
            }),
            grade: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("如：三年级")
                    .default_value(g)
            }),
            school: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("就读学校")
                    .default_value(s)
            }),
            term_start,
            term_end,
        })
    }

    fn snapshot(&self, cx: &gpui::App) -> Profile {
        let current = self.app.read(cx).db.profile.clone();
        Profile {
            student: self.name.read(cx).value().to_string(),
            grade: self.grade.read(cx).value().to_string(),
            school: self.school.read(cx).value().to_string(),
            term_start: ymd_or(self.term_start.read(cx).date_time(), &current.term_start),
            term_end: ymd_or(self.term_end.read(cx).date_time(), &current.term_end),
            reminder_enabled: current.reminder_enabled,
            auto_load_today: current.auto_load_today,
            sync_ts: current.sync_ts,
        }
    }
}

impl Render for ProfileInputs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let name = self.name.clone();
        let grade = self.grade.clone();
        let school = self.school.clone();
        let term_start = self.term_start.clone();
        let term_end = self.term_end.clone();

        div()
            .flex()
            .flex_col()
            .gap(SPACE_MD)
            .child(field_row("姓名", Input::new(&name).h(INPUT_H).w_full()))
            .child(field_row("年级", Input::new(&grade).h(INPUT_H).w_full()))
            .child(field_row("学校", Input::new(&school).h(INPUT_H).w_full()))
            .child(
                div()
                    .flex()
                    .gap(SPACE_MD)
                    .child(div().flex_1().min_w_0().child(field_row(
                        "学期开始",
                        DatePicker::new(&term_start).placeholder("选择日期"),
                    )))
                    .child(div().flex_1().min_w_0().child(field_row(
                        "学期结束",
                        DatePicker::new(&term_end).placeholder("选择日期"),
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(SPACE_SM)
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child("日期点选即可；改完点右侧保存。"),
                    )
                    .child(btn::secondary_with_icon(
                        "save-profile",
                        "保存资料",
                        IconName::Save,
                        move |_e, window, app| {
                            let (values, owner) = {
                                let guard = this.read(app);
                                (guard.snapshot(app), guard.app.clone())
                            };
                            owner.update(app, |root, cx| {
                                root.db.save_profile(values);
                                root.commit(window, cx);
                            });
                        },
                    )),
            )
    }
}

pub fn render(app: &mut App, window: &mut Window, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();

    // 编辑器只创建一次，避免重新渲染时丢失正在输入的内容。
    let profile = match &app.profile_inputs {
        Some(entity) => entity.clone(),
        None => {
            let entity = ProfileInputs::new(&app.db.profile, cx.entity(), window, cx);
            app.profile_inputs = Some(entity.clone());
            entity
        }
    };

    let reminders_on = app.db.profile.reminder_enabled;
    let auto_today = app.db.profile.auto_load_today;
    let dark = theme.is_dark();

    let this_rem = this.clone();
    let reminder_switch = Switch::new("switch-reminder")
        .checked(reminders_on)
        .label("提醒到期时弹出系统通知")
        .on_change(move |checked: &bool, window, cx| {
            this_rem.update(cx, |this, cx| {
                let mut profile = this.db.profile.clone();
                profile.reminder_enabled = *checked;
                this.db.save_profile(profile);
                this.commit(window, cx);
            });
        });

    let this_auto = this.clone();
    let auto_switch = Switch::new("switch-auto")
        .checked(auto_today)
        .label("启动时打开今日概览")
        .on_change(move |checked: &bool, window, cx| {
            this_auto.update(cx, |this, cx| {
                let mut profile = this.db.profile.clone();
                profile.auto_load_today = *checked;
                this.db.save_profile(profile);
                this.commit(window, cx);
            });
        });

    // 外观主题（§4.8-3）：浅色 / 深色 tab_chips，点击即时切换（不落盘）。
    let theme_tabs = tab_chips(
        &theme,
        "theme-mode",
        &[("浅色", ThemeMode::Light), ("深色", ThemeMode::Dark)],
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        move |mode, window, cx| {
            Theme::change(mode, Some(window), cx);
        },
    );

    let this_clear = this.clone();
    let clear_button = btn::danger_with_icon(
        "clear-data",
        "清空全部数据",
        IconName::Trash,
        move |_e, window, cx| {
            confirm_clear_all(this_clear.clone(), window, cx);
        },
    );

    let this_sample = this.clone();
    let sample_button = btn::secondary_with_icon(
        "load-sample",
        "载入示例数据",
        IconName::Plus,
        move |_e, window, cx| {
            confirm_load_sample(this_sample.clone(), window, cx);
        },
    );

    let sync_card = render_sync_card(app, window, cx);

    let data_path = crate::store::data_file().display().to_string();
    let data_card = card_with_theme(
        &theme,
        CardVariant::Default,
        Some("数据存储"),
        Some("数据保存在本地，不联网"),
        div()
            .flex()
            .flex_col()
            .gap(SPACE_SM)
            .child(kv_row(&theme, "数据文件", data_path))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(SPACE_SM)
                    .child(
                        div()
                            .id(eid("data-action", "open-dir"))
                            .flex_shrink_0()
                            .test_support()
                            .child(btn::link(
                                "open-data-dir",
                                "打开数据目录",
                                |_e, _w, cx| {
                                    cx.reveal_path(&crate::store::data_dir());
                                },
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child("在文件管理器里打开"),
                    ),
            )
            .child(kv_row(
                &theme,
                "课程",
                format!("{} 门", app.db.live_course_count()),
            ))
            .child(kv_row(
                &theme,
                "课表条目",
                format!("{} 条", app.db.live_session_count()),
            ))
            .child(kv_row(
                &theme,
                "日程安排",
                format!("{} 条", app.db.live_event_count()),
            ))
            .child(kv_row(
                &theme,
                "提醒事项",
                format!("{} 条", app.db.live_reminder_count()),
            ))
            .child(kv_row(
                &theme,
                "打卡任务",
                format!("{} 个", app.db.live_checkin_count()),
            ))
            .child(kv_row(
                &theme,
                "备忘笔记",
                format!("{} 条", app.db.live_memo_count()),
            ))
            .child(div().h(SPACE_XS))
            .child(
                div()
                    .flex()
                    .gap(SPACE_SM)
                    .flex_wrap()
                    .child(sample_button)
                    .child(clear_button),
            ),
    );

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap(SPACE_LG)
        .overflow_y_scrollbar()
        .child(card_with_theme(
            &theme,
            CardVariant::Default,
            Some("学生信息"),
            Some("用于首页问候与统计"),
            profile,
        ))
        .child(card_with_theme(
            &theme,
            CardVariant::Default,
            Some("提醒与启动"),
            None::<SharedString>,
            div()
                .flex()
                .flex_col()
                .gap(SPACE_LG)
                .child(reminder_switch)
                .child(auto_switch),
        ))
        .child(card_with_theme(
            &theme,
            CardVariant::Default,
            Some("外观主题"),
            Some("切换后立即生效"),
            div().flex().child(theme_tabs),
        ))
        .child(sync_card)
        .child(data_card)
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child("小学课程管理计划 · 基于 GPUI Kit 构建"),
        )
}

/// 同步卡片（§4.8-4）。
///
/// **Phase 4 V3 仅改样式**：输入框、四个按钮（登录 / 注册 / 登出 / 立即同步）、
/// 状态行与 Phase 3 的行为、代码路径完全一致；同步 worker 与通知逻辑不在本页。
fn render_sync_card(app: &mut App, window: &mut Window, cx: &mut Context<App>) -> AnyElement {
    let theme = cx.theme().clone();
    let this = cx.entity();

    let server_url = match &app.sync_server_url {
        Some(entity) => entity.clone(),
        None => {
            let entity =
                cx.new(|cx| InputState::new(window, cx).placeholder("http://127.0.0.1:8080"));
            app.sync_server_url = Some(entity.clone());
            entity
        }
    };
    let username = match &app.sync_username {
        Some(entity) => entity.clone(),
        None => {
            let entity = cx.new(|cx| InputState::new(window, cx).placeholder("用户名"));
            app.sync_username = Some(entity.clone());
            entity
        }
    };
    let password = match &app.sync_password {
        Some(entity) => entity.clone(),
        None => {
            let entity = cx.new(|cx| InputState::new(window, cx).placeholder("密码").masked(true));
            app.sync_password = Some(entity.clone());
            entity
        }
    };

    let login = {
        let this = this.clone();
        let url = server_url.clone();
        let user = username.clone();
        let pass = password.clone();
        btn::primary_with_icon(
            "sync-login",
            "登录",
            IconName::LogIn,
            move |_e, _window, cx| {
                let (url, user, pass) = (
                    url.read(cx).value().to_string(),
                    user.read(cx).value().to_string(),
                    pass.read(cx).unmask_value().to_string(),
                );
                this.update(cx, |this, _cx| {
                    this.sync.send(SyncCommand::Login {
                        server_url: url,
                        username: user,
                        password: pass,
                        register: false,
                    });
                });
            },
        )
    };
    let register = {
        let this = this.clone();
        let url = server_url.clone();
        let user = username.clone();
        let pass = password.clone();
        btn::secondary_with_icon(
            "sync-register",
            "注册",
            IconName::UserRound,
            move |_e, _window, cx| {
                let (url, user, pass) = (
                    url.read(cx).value().to_string(),
                    user.read(cx).value().to_string(),
                    pass.read(cx).unmask_value().to_string(),
                );
                this.update(cx, |this, _cx| {
                    this.sync.send(SyncCommand::Login {
                        server_url: url,
                        username: user,
                        password: pass,
                        register: true,
                    });
                });
            },
        )
    };
    let logout = {
        let this = this.clone();
        btn::secondary_with_icon(
            "sync-logout",
            "登出",
            IconName::LogOut,
            move |_e, _window, cx| {
                this.update(cx, |this, cx| this.sync_logout(cx));
            },
        )
    };
    let sync_now = {
        let this = this.clone();
        btn::secondary_with_icon(
            "sync-now",
            "立即同步",
            IconName::RefreshCw,
            move |_e, _window, cx| {
                this.update(cx, |this, _cx| this.sync.send(SyncCommand::SyncNow));
            },
        )
    };

    let status_text = sync_status_text(&app.sync_status, app.sync_notice.as_deref());

    // QA/视觉证据锚点（§6.8 风格）：外层薄壳让按钮可被 `window.click(id)` 命中。
    let action = |key: &str, button: Button| {
        div()
            .id(eid("sync-action", key))
            .flex_shrink_0()
            .test_support()
            .child(button)
    };

    div()
        .id("sync-card")
        .flex()
        .flex_col()
        .test_support()
        .child(card_with_theme(
            &theme,
            CardVariant::Default,
            Some("同步"),
            Some("多设备同步（可选，离线也能用）"),
            div()
                .flex()
                .flex_col()
                .gap(SPACE_MD)
                .child(field_row(
                    "服务器地址",
                    Input::new(&server_url).h(INPUT_H).w_full(),
                ))
                .child(field_row(
                    "用户名",
                    Input::new(&username).h(INPUT_H).w_full(),
                ))
                .child(field_row("密码", Input::new(&password).h(INPUT_H).w_full()))
                .child(kv_row(&theme, "状态", status_text))
                .child(
                    div()
                        .flex()
                        .gap(SPACE_SM)
                        .flex_wrap()
                        .child(action("login", login))
                        .child(action("register", register))
                        .child(action("logout", logout))
                        .child(action("sync-now", sync_now)),
                ),
        ))
        .into_any_element()
}

/// 清空数据确认对话框。
fn confirm_clear_all(owner: Entity<App>, window: &mut Window, cx: &mut gpui::App) {
    window.open_alert_dialog(cx, move |alert, _w, _cx| {
        let owner = owner.clone();
        alert
            .title("清空数据")
            .description("课程、课表、日程、提醒、打卡与备忘都会被删除，且无法恢复。")
            .ok_text("确认清空")
            .ok_variant(ButtonVariant::Danger)
            .on_ok(move |_e, window, cx| {
                owner.update(cx, |this, cx| {
                    this.db.clear_all();
                    this.commit(window, cx);
                    window.push_notification(Notification::success("数据已清空").title("完成"), cx);
                });
                true
            })
    });
}

/// 载入示例数据确认对话框。
fn confirm_load_sample(owner: Entity<App>, window: &mut Window, cx: &mut gpui::App) {
    window.open_alert_dialog(cx, move |alert, _w, _cx| {
        let owner = owner.clone();
        alert
            .title("载入示例数据")
            .description("将清空现有数据并载入一套示例内容（课程、课表、打卡与备忘）。继续吗？")
            .ok_text("载入示例")
            .on_ok(move |_e, window, cx| {
                owner.update(cx, |this, cx| {
                    this.db.load_sample_data();
                    this.commit(window, cx);
                    window.push_notification(
                        Notification::success("示例数据已载入").title("完成"),
                        cx,
                    );
                });
                true
            })
    });
}

/// 同步状态 + 提示文案（设置页状态行）。
fn sync_status_text(status: &Status, notice: Option<&str>) -> String {
    let base = match status {
        Status::Idle { last_sync_ms } => match last_sync_ms {
            Some(ms) => chrono::DateTime::from_timestamp_millis(*ms)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
                .map(|time| format!("已同步 · {time}"))
                .unwrap_or_else(|| "已同步".to_string()),
            None => "已同步".to_string(),
        },
        other => other.label_cn().to_string(),
    };
    match notice {
        Some(note) if !note.is_empty() => format!("{base} · {note}"),
        _ => base,
    }
}

/// "2026-09-01" → `NaiveDate`（无法解析时 `None`，此时日期控件保持空态）。
fn parse_ymd(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()
}

/// 日期控件值 → "YYYY-MM-DD"。未选择日期时回落到原值（不清空既有资料）。
fn ymd_or(value: DateTime, fallback: &str) -> String {
    value
        .format("%Y-%m-%d")
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback.to_string())
}
