use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::ThemeMode;
use gpui::component::WindowExt;
use gpui::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui::component::input::{Input, InputState};
use gpui::component::notification::Notification;
use gpui::component::scroll::ScrollableElement;
use gpui::component::switch::Switch;
use gpui::*;

use crate::app::App;
use crate::model::Profile;
use crate::ui::form::field_row;
use crate::ui::widgets::{card, eid, kv_row, panel, section_title};

/// 学生资料编辑区。作为独立视图，方便读取输入值并写回 App。
pub struct ProfileInputs {
    app: Entity<App>,
    name: Entity<InputState>,
    grade: Entity<InputState>,
    school: Entity<InputState>,
    term_start: Entity<InputState>,
    term_end: Entity<InputState>,
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
        let ts = profile.term_start.clone();
        let te = profile.term_end.clone();
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
            term_start: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("2026-09-01")
                    .default_value(ts)
            }),
            term_end: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("2027-01-20")
                    .default_value(te)
            }),
        })
    }

    fn snapshot(&self, cx: &gpui::App) -> Profile {
        let current = self.app.read(cx).db.profile.clone();
        Profile {
            student: self.name.read(cx).value().to_string(),
            grade: self.grade.read(cx).value().to_string(),
            school: self.school.read(cx).value().to_string(),
            term_start: self.term_start.read(cx).value().to_string(),
            term_end: self.term_end.read(cx).value().to_string(),
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
            .gap_3()
            .child(field_row("姓名", Input::new(&name).h(px(32.)).w_full()))
            .child(field_row("年级", Input::new(&grade).h(px(32.)).w_full()))
            .child(field_row("学校", Input::new(&school).h(px(32.)).w_full()))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(field_row(
                        "学期开始",
                        Input::new(&term_start).h(px(32.)).w_full(),
                    )))
                    .child(div().flex_1().child(field_row(
                        "学期结束",
                        Input::new(&term_end).h(px(32.)).w_full(),
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("日期格式 2026-09-01"),
                    )
                    .child(
                        Button::new("save-profile")
                            .secondary()
                            .compact()
                            .label("保存资料")
                            .on_click(move |_e, window, app| {
                                let (values, owner) = {
                                    let guard = this.read(app);
                                    (guard.snapshot(app), guard.app.clone())
                                };
                                owner.update(app, |root, cx| {
                                    root.db.save_profile(values);
                                    root.commit(window, cx);
                                });
                            }),
                    ),
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

    let mut theme_row = div().flex().gap_2();
    for (label, icon, mode, active) in [
        ("浅色", IconName::Sun, ThemeMode::Light, !dark),
        ("深色", IconName::Moon, ThemeMode::Dark, dark),
    ] {
        theme_row = theme_row.child(
            div()
                .id(eid("theme-mode", label))
                .px_4()
                .py(px(6.))
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .border_1()
                .border_color(if active { theme.primary } else { theme.border })
                .bg(if active {
                    theme.primary.with_alpha(0.12)
                } else {
                    panel(&theme)
                })
                .text_color(if active {
                    theme.primary
                } else {
                    theme.foreground
                })
                .on_click(move |_e, window, cx| {
                    Theme::change(mode, Some(window), cx);
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(icon).with_size(Size::XSmall))
                        .child(label),
                ),
        );
    }

    let this_clear = this.clone();
    let clear_button = Button::new("clear-data")
        .danger()
        .compact()
        .icon(Icon::new(IconName::Trash))
        .label("清空全部数据")
        .on_click(move |_e, window, cx| {
            confirm_clear_all(this_clear.clone(), window, cx);
        });

    let this_sample = this.clone();
    let sample_button = Button::new("load-sample")
        .secondary()
        .compact()
        .icon(Icon::new(IconName::Plus))
        .label("载入示例数据")
        .on_click(move |_e, window, cx| {
            confirm_load_sample(this_sample.clone(), window, cx);
        });

    let data_path = crate::store::data_file().display().to_string();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap_4()
        .overflow_y_scrollbar()
        .child(section_title(&theme, "设置", None))
        .child(card(
            &theme,
            "学生信息",
            Some("用于首页问候与统计"),
            profile,
        ))
        .child(card(
            &theme,
            "提醒与启动",
            None::<SharedString>,
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(reminder_switch)
                .child(auto_switch),
        ))
        .child(card(&theme, "外观主题", None::<SharedString>, theme_row))
        .child(card(
            &theme,
            "数据存储",
            Some("数据保存在本地，不联网"),
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(kv_row(&theme, "数据文件", data_path))
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
                .child(div().h(px(4.)))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .flex_wrap()
                        .child(sample_button)
                        .child(clear_button),
                ),
        ))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("小学课程管理计划 · 基于 GPUI Kit 构建"),
        )
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
