use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, Timelike, Weekday};
use course_plan_sync_proto::{Change, EntityKind, wins};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const WEEKDAY_LABELS: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
pub const WEEKDAY_SHORT: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

/// 学校课程与补习课程的类型。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CourseKind {
    School,
    Tutoring,
}

impl CourseKind {
    pub fn label(self) -> &'static str {
        match self {
            CourseKind::School => "学校课程",
            CourseKind::Tutoring => "补习课程",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            CourseKind::School => "校内",
            CourseKind::Tutoring => "补习",
        }
    }
}

/// 一门课程。颜色以索引存储，方便整体换肤。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Course {
    pub id: String,
    pub name: String,
    pub kind: CourseKind,
    pub subject: String,
    pub teacher: String,
    pub location: String,
    pub note: String,
    pub color: usize,
    pub active: bool,
    #[serde(default)]
    pub sync_ts: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

impl Course {
    pub fn new(name: impl Into<String>, kind: CourseKind) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            kind,
            subject: String::new(),
            teacher: String::new(),
            location: String::new(),
            note: String::new(),
            color: 0,
            active: true,
            sync_ts: now_ms(),
            deleted_at: None,
        }
    }
}

/// 课表中的一条每周固定安排。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub course_id: String,
    /// 1 = 周一 … 7 = 周日
    pub weekday: u8,
    /// "08:00"
    pub start: String,
    /// "08:45"
    pub end: String,
    pub room: String,
    #[serde(default)]
    pub sync_ts: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

impl Session {
    pub fn new(
        course_id: impl Into<String>,
        weekday: u8,
        start: impl Into<String>,
        end: impl Into<String>,
        room: impl Into<String>,
    ) -> Self {
        Self {
            id: new_id(),
            course_id: course_id.into(),
            weekday,
            start: start.into(),
            end: end.into(),
            room: room.into(),
            sync_ts: now_ms(),
            deleted_at: None,
        }
    }

    pub fn start_time(&self) -> NaiveTime {
        parse_time(&self.start).unwrap_or(NaiveTime::from_hms_opt(8, 0, 0).unwrap())
    }

    pub fn end_time(&self) -> NaiveTime {
        parse_time(&self.end).unwrap_or(NaiveTime::from_hms_opt(8, 45, 0).unwrap())
    }

    /// 分钟数形式的时刻，便于排序与求差。
    pub fn start_minutes(&self) -> i32 {
        time_minutes(self.start_time())
    }

    pub fn end_minutes(&self) -> i32 {
        time_minutes(self.end_time())
    }
}

pub fn parse_time(text: &str) -> Option<NaiveTime> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let h: u32 = parts[0].trim().parse().ok()?;
    let m: u32 = parts[1].trim().parse().ok()?;
    NaiveTime::from_hms_opt(h, m, 0)
}

pub fn format_time(t: NaiveTime) -> String {
    format!("{:02}:{:02}", t.hour(), t.minute())
}

pub fn time_minutes(t: NaiveTime) -> i32 {
    t.hour() as i32 * 60 + t.minute() as i32
}

/// 一次性的日程安排，与课程关联后会显示课程名。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventItem {
    pub id: String,
    pub title: String,
    pub date: String,
    pub start: String,
    pub end: String,
    pub location: String,
    pub course_id: Option<String>,
    pub note: String,
    pub done: bool,
    #[serde(default)]
    pub sync_ts: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

impl EventItem {
    pub fn new(title: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            title: title.into(),
            date: date.into(),
            start: "09:00".into(),
            end: "10:00".into(),
            location: String::new(),
            course_id: None,
            note: String::new(),
            done: false,
            sync_ts: now_ms(),
            deleted_at: None,
        }
    }
}

/// 日程提醒。
///
/// 注意：「已通知」记账（原 `notified_for`）为**设备本地数据**，存于
/// `runtime_state.json`（见 [`crate::store::RuntimeState`]），不进同步载荷。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub content: String,
    /// "2026-03-05 08:00"
    pub due_at: String,
    pub repeat: Repeat,
    pub done: bool,
    #[serde(default)]
    pub sync_ts: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

impl Reminder {
    pub fn new(title: impl Into<String>, due_at: impl Into<String>, repeat: Repeat) -> Self {
        Self {
            id: new_id(),
            title: title.into(),
            content: String::new(),
            due_at: due_at.into(),
            repeat,
            done: false,
            sync_ts: now_ms(),
            deleted_at: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Repeat {
    Once,
    Daily,
    Weekly,
}

impl Repeat {
    pub fn label(self) -> &'static str {
        match self {
            Repeat::Once => "仅一次",
            Repeat::Daily => "每天",
            Repeat::Weekly => "每周",
        }
    }
}

/// 打卡任务。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckIn {
    pub id: String,
    pub title: String,
    pub note: String,
    pub repeat: Repeat,
    /// Repeat::Weekly 时生效，1 = 周一 … 7 = 周日
    pub weekdays: Vec<u8>,
    /// Repeat::Once 时生效
    pub date: String,
    pub archived: bool,
    /// "2026-03-05" -> true
    pub history: BTreeMap<String, bool>,
    pub created_at: String,
    #[serde(default)]
    pub sync_ts: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

impl CheckIn {
    pub fn new(title: impl Into<String>, repeat: Repeat) -> Self {
        Self {
            id: new_id(),
            title: title.into(),
            note: String::new(),
            repeat,
            weekdays: Vec::new(),
            date: fmt_date(today()),
            archived: false,
            history: BTreeMap::new(),
            created_at: now_string(),
            sync_ts: now_ms(),
            deleted_at: None,
        }
    }

    /// 某一天是否需要打卡。
    pub fn active_on(&self, date: NaiveDate) -> bool {
        if self.archived {
            return false;
        }
        match self.repeat {
            Repeat::Once => self.date == date.format("%Y-%m-%d").to_string(),
            Repeat::Daily => true,
            Repeat::Weekly => {
                self.weekdays.contains(&iso_weekday(date)) || self.weekdays.is_empty()
            }
        }
    }

    pub fn done_on(&self, date: NaiveDate) -> bool {
        *self
            .history
            .get(date.format("%Y-%m-%d").to_string().as_str())
            .unwrap_or(&false)
    }

    pub fn set_done(&mut self, date: NaiveDate, done: bool) {
        self.history
            .insert(date.format("%Y-%m-%d").to_string(), done);
    }

    /// 连续打卡天数（从今天或昨天往前推）。
    pub fn streak(&self) -> usize {
        let today = today();
        let mut cursor = if self.done_on(today) {
            today
        } else {
            today - Duration::days(1)
        };
        let mut count = 0;
        while self.active_on(cursor) && self.done_on(cursor) && count < 999 {
            count += 1;
            cursor -= Duration::days(1);
        }
        count
    }

    pub fn total_done(&self) -> usize {
        self.history.values().filter(|v| **v).count()
    }

    pub fn repeat_label(&self) -> String {
        match self.repeat {
            Repeat::Once => format!("仅 {}", self.date),
            Repeat::Daily => "每天".to_string(),
            Repeat::Weekly => {
                if self.weekdays.is_empty() {
                    "每周".to_string()
                } else {
                    let mut d = self.weekdays.clone();
                    d.sort();
                    d.iter()
                        .map(|w| WEEKDAY_SHORT[(*w as usize).clamp(1, 7) - 1])
                        .collect::<Vec<_>>()
                        .join("、")
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memo {
    pub id: String,
    pub title: String,
    pub content: String,
    pub color: usize,
    pub pinned: bool,
    pub created_at: String,
    /// 显示用字符串；**不是**同步戳（同步戳为 `sync_ts`）。
    pub updated_at: String,
    #[serde(default)]
    pub sync_ts: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

impl Memo {
    pub fn new() -> Self {
        let now = now_string();
        Self {
            id: new_id(),
            title: String::new(),
            content: String::new(),
            color: 0,
            pinned: false,
            created_at: now.clone(),
            updated_at: now,
            sync_ts: now_ms(),
            deleted_at: None,
        }
    }
}

impl Default for Memo {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub student: String,
    pub grade: String,
    pub school: String,
    /// 学期开始日期 "2026-02-23"
    pub term_start: String,
    pub term_end: String,
    pub reminder_enabled: bool,
    pub auto_load_today: bool,
    /// 单例 profile 的同步戳（profile 无墓碑）。
    #[serde(default)]
    pub sync_ts: i64,
}

impl Default for Profile {
    fn default() -> Self {
        let today = today();
        Self {
            student: String::new(),
            grade: String::new(),
            school: String::new(),
            term_start: (today - Duration::days(30)).format("%Y-%m-%d").to_string(),
            term_end: (today + Duration::days(120)).format("%Y-%m-%d").to_string(),
            reminder_enabled: true,
            auto_load_today: true,
            sync_ts: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Database {
    #[serde(default)]
    pub profile: Profile,
    #[serde(default)]
    pub courses: Vec<Course>,
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub events: Vec<EventItem>,
    #[serde(default)]
    pub reminders: Vec<Reminder>,
    #[serde(default)]
    pub checkins: Vec<CheckIn>,
    #[serde(default)]
    pub memos: Vec<Memo>,
    /// 推送水位（协议 §2.1 的 `push_watermark_ms`）；本地戳下限。仅内存，不落盘。
    #[serde(skip)]
    pub stamp_floor_ms: i64,
    /// 本轮同步（[`Database::begin_round`] 起）内最小本地改动戳；仅内存，不落盘。
    #[serde(skip)]
    pub round_min_mut_ts: Option<i64>,
}

impl Database {
    // MARK: - 读取（全部过滤墓碑）

    pub fn live_courses(&self) -> impl Iterator<Item = &Course> {
        self.courses.iter().filter(|c| c.deleted_at.is_none())
    }

    pub fn live_sessions(&self) -> impl Iterator<Item = &Session> {
        self.sessions.iter().filter(|s| s.deleted_at.is_none())
    }

    pub fn live_events(&self) -> impl Iterator<Item = &EventItem> {
        self.events.iter().filter(|e| e.deleted_at.is_none())
    }

    pub fn live_reminders(&self) -> impl Iterator<Item = &Reminder> {
        self.reminders.iter().filter(|r| r.deleted_at.is_none())
    }

    pub fn live_checkins(&self) -> impl Iterator<Item = &CheckIn> {
        self.checkins.iter().filter(|c| c.deleted_at.is_none())
    }

    pub fn live_memos(&self) -> impl Iterator<Item = &Memo> {
        self.memos.iter().filter(|m| m.deleted_at.is_none())
    }

    pub fn live_course_count(&self) -> usize {
        self.live_courses().count()
    }

    pub fn live_session_count(&self) -> usize {
        self.live_sessions().count()
    }

    pub fn live_event_count(&self) -> usize {
        self.live_events().count()
    }

    pub fn live_reminder_count(&self) -> usize {
        self.live_reminders().count()
    }

    pub fn live_checkin_count(&self) -> usize {
        self.live_checkins().count()
    }

    pub fn live_memo_count(&self) -> usize {
        self.live_memos().count()
    }

    pub fn course(&self, id: &str) -> Option<&Course> {
        self.courses
            .iter()
            .find(|c| c.id == id && c.deleted_at.is_none())
    }

    pub fn course_name(&self, id: &str) -> String {
        self.course(id).map(|c| c.name.clone()).unwrap_or_default()
    }

    /// 指定星期几的课程安排，按开始时间排序。
    pub fn sessions_on(&self, weekday: u8) -> Vec<&Session> {
        let mut list: Vec<&Session> = self
            .sessions
            .iter()
            .filter(|s| s.deleted_at.is_none() && s.weekday == weekday)
            .collect();
        list.sort_by_key(|s| (s.start_minutes(), s.end_minutes()));
        list
    }

    /// 一周的总课时（分钟）。
    pub fn weekly_minutes(&self) -> i64 {
        self.live_sessions()
            .map(|s| (s.end_minutes() - s.start_minutes()).max(0) as i64)
            .sum()
    }

    pub fn sessions_of(&self, course_id: &str) -> Vec<&Session> {
        let mut list: Vec<&Session> = self
            .sessions
            .iter()
            .filter(|s| s.deleted_at.is_none() && s.course_id == course_id)
            .collect();
        list.sort_by_key(|s| (s.weekday, s.start_minutes()));
        list
    }

    pub fn reminders_on(&self, date: NaiveDate) -> Vec<&Reminder> {
        let key = date.format("%Y-%m-%d").to_string();
        let mut list: Vec<&Reminder> = self
            .live_reminders()
            .filter(|r| {
                if r.done && r.repeat == Repeat::Once {
                    return false;
                }
                let Ok(due) =
                    NaiveDate::parse_from_str(r.due_at.get(..10).unwrap_or(""), "%Y-%m-%d")
                else {
                    return false;
                };
                match r.repeat {
                    Repeat::Once => due == date,
                    Repeat::Daily => due <= date,
                    Repeat::Weekly => due <= date && (date - due).num_days() % 7 == 0,
                }
            })
            .collect();
        list.sort_by_key(|r| r.due_at.clone());
        let _ = key;
        list
    }

    pub fn events_on(&self, date: NaiveDate) -> Vec<&EventItem> {
        let key = date.format("%Y-%m-%d").to_string();
        let mut list: Vec<&EventItem> = self.live_events().filter(|e| e.date == key).collect();
        list.sort_by_key(|e| e.start.clone());
        list
    }

    pub fn checkins_on(&self, date: NaiveDate) -> Vec<&CheckIn> {
        self.live_checkins().filter(|c| c.active_on(date)).collect()
    }

    // MARK: - 戳规则（协议 §2.1）

    /// 计算并记录一个单调递增的本地改动戳：
    /// `ts = max(now_ms, stamp_floor_ms + 1, prev + 1)`。
    ///
    /// 同时维护本轮最小改动戳（供同步引擎推进水位下限，协议 §5.3 R3）。
    fn next_stamp(&mut self, prev: i64) -> i64 {
        let ts = now_ms()
            .max(self.stamp_floor_ms.saturating_add(1))
            .max(prev.saturating_add(1));
        self.round_min_mut_ts = Some(match self.round_min_mut_ts {
            Some(min) => min.min(ts),
            None => ts,
        });
        ts
    }

    /// 同步引擎在轮次开始时调用：重置本轮最小改动戳记账。
    pub fn begin_round(&mut self, _round_start_ms: i64) {
        self.round_min_mut_ts = None;
    }

    /// 本轮（自 [`Database::begin_round`] 起）记录到的最小本地改动戳。
    pub fn round_min_mutation_ts(&self) -> Option<i64> {
        self.round_min_mut_ts
    }

    // MARK: - Mutation API（本地改动 = 单调重戳）

    pub fn save_course(&mut self, mut course: Course) {
        let prev = self
            .courses
            .iter()
            .find(|c| c.id == course.id)
            .map(|c| c.sync_ts)
            .unwrap_or(0);
        course.sync_ts = self.next_stamp(prev);
        course.deleted_at = None;
        match self.courses.iter_mut().find(|c| c.id == course.id) {
            Some(slot) => *slot = course,
            None => self.courses.push(course),
        }
    }

    /// 删除课程：盖墓碑，并级联把该课程下所有**存活**课表安排盖同一墓碑戳。
    pub fn delete_course(&mut self, id: &str) {
        let Some(prev) = self.courses.iter().find(|c| c.id == id).map(|c| c.sync_ts) else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(course) = self.courses.iter_mut().find(|c| c.id == id) {
            course.sync_ts = ts;
            course.deleted_at = Some(ts);
        }
        for session in self
            .sessions
            .iter_mut()
            .filter(|s| s.course_id == id && s.deleted_at.is_none())
        {
            session.sync_ts = ts;
            session.deleted_at = Some(ts);
        }
    }

    pub fn save_session(&mut self, mut session: Session) {
        let prev = self
            .sessions
            .iter()
            .find(|s| s.id == session.id)
            .map(|s| s.sync_ts)
            .unwrap_or(0);
        session.sync_ts = self.next_stamp(prev);
        session.deleted_at = None;
        match self.sessions.iter_mut().find(|s| s.id == session.id) {
            Some(slot) => *slot = session,
            None => self.sessions.push(session),
        }
    }

    pub fn delete_session(&mut self, id: &str) {
        let Some(prev) = self.sessions.iter().find(|s| s.id == id).map(|s| s.sync_ts) else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
            session.sync_ts = ts;
            session.deleted_at = Some(ts);
        }
    }

    pub fn save_event(&mut self, mut event: EventItem) {
        let prev = self
            .events
            .iter()
            .find(|e| e.id == event.id)
            .map(|e| e.sync_ts)
            .unwrap_or(0);
        event.sync_ts = self.next_stamp(prev);
        event.deleted_at = None;
        match self.events.iter_mut().find(|e| e.id == event.id) {
            Some(slot) => *slot = event,
            None => self.events.push(event),
        }
    }

    pub fn delete_event(&mut self, id: &str) {
        let Some(prev) = self.events.iter().find(|e| e.id == id).map(|e| e.sync_ts) else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(event) = self.events.iter_mut().find(|e| e.id == id) {
            event.sync_ts = ts;
            event.deleted_at = Some(ts);
        }
    }

    pub fn set_event_done(&mut self, id: &str, done: bool) {
        let Some(prev) = self.events.iter().find(|e| e.id == id).map(|e| e.sync_ts) else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(event) = self.events.iter_mut().find(|e| e.id == id) {
            event.sync_ts = ts;
            event.done = done;
        }
    }

    pub fn save_reminder(&mut self, mut reminder: Reminder) {
        let prev = self
            .reminders
            .iter()
            .find(|r| r.id == reminder.id)
            .map(|r| r.sync_ts)
            .unwrap_or(0);
        reminder.sync_ts = self.next_stamp(prev);
        reminder.deleted_at = None;
        match self.reminders.iter_mut().find(|r| r.id == reminder.id) {
            Some(slot) => *slot = reminder,
            None => self.reminders.push(reminder),
        }
    }

    pub fn delete_reminder(&mut self, id: &str) {
        let Some(prev) = self
            .reminders
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.sync_ts)
        else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(reminder) = self.reminders.iter_mut().find(|r| r.id == id) {
            reminder.sync_ts = ts;
            reminder.deleted_at = Some(ts);
        }
    }

    pub fn set_reminder_done(&mut self, id: &str, done: bool) {
        let Some(prev) = self
            .reminders
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.sync_ts)
        else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(reminder) = self.reminders.iter_mut().find(|r| r.id == id) {
            reminder.sync_ts = ts;
            reminder.done = done;
        }
    }

    pub fn save_checkin(&mut self, mut checkin: CheckIn) {
        let prev = self
            .checkins
            .iter()
            .find(|c| c.id == checkin.id)
            .map(|c| c.sync_ts)
            .unwrap_or(0);
        checkin.sync_ts = self.next_stamp(prev);
        checkin.deleted_at = None;
        match self.checkins.iter_mut().find(|c| c.id == checkin.id) {
            Some(slot) => *slot = checkin,
            None => self.checkins.push(checkin),
        }
    }

    pub fn delete_checkin(&mut self, id: &str) {
        let Some(prev) = self.checkins.iter().find(|c| c.id == id).map(|c| c.sync_ts) else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(checkin) = self.checkins.iter_mut().find(|c| c.id == id) {
            checkin.sync_ts = ts;
            checkin.deleted_at = Some(ts);
        }
    }

    pub fn set_checkin_done(&mut self, id: &str, date: NaiveDate, done: bool) {
        let Some(prev) = self.checkins.iter().find(|c| c.id == id).map(|c| c.sync_ts) else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(checkin) = self.checkins.iter_mut().find(|c| c.id == id) {
            checkin.sync_ts = ts;
            checkin.set_done(date, done);
        }
    }

    pub fn set_checkin_archived(&mut self, id: &str, archived: bool) {
        let Some(prev) = self.checkins.iter().find(|c| c.id == id).map(|c| c.sync_ts) else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(checkin) = self.checkins.iter_mut().find(|c| c.id == id) {
            checkin.sync_ts = ts;
            checkin.archived = archived;
        }
    }

    pub fn save_memo(&mut self, mut memo: Memo) {
        let prev = self
            .memos
            .iter()
            .find(|m| m.id == memo.id)
            .map(|m| m.sync_ts)
            .unwrap_or(0);
        memo.sync_ts = self.next_stamp(prev);
        memo.deleted_at = None;
        match self.memos.iter_mut().find(|m| m.id == memo.id) {
            Some(slot) => {
                // 显示字段刷新，但 created_at 保持不变（协议 §2.1 备注）。
                memo.created_at = slot.created_at.clone();
                memo.updated_at = now_string();
                *slot = memo;
            }
            None => {
                memo.updated_at = now_string();
                self.memos.push(memo);
            }
        }
    }

    pub fn delete_memo(&mut self, id: &str) {
        let Some(prev) = self.memos.iter().find(|m| m.id == id).map(|m| m.sync_ts) else {
            return;
        };
        let ts = self.next_stamp(prev);
        if let Some(memo) = self.memos.iter_mut().find(|m| m.id == id) {
            memo.sync_ts = ts;
            memo.deleted_at = Some(ts);
        }
    }

    pub fn save_profile(&mut self, mut profile: Profile) {
        let ts = self.next_stamp(self.profile.sync_ts);
        profile.sync_ts = ts;
        self.profile = profile;
    }

    /// 清空全部数据：给全部**存活**实体盖同一墓碑戳（profile 无墓碑，保留）。
    pub fn clear_all(&mut self) {
        let has_live = self.courses.iter().any(|c| c.deleted_at.is_none())
            || self.sessions.iter().any(|s| s.deleted_at.is_none())
            || self.events.iter().any(|e| e.deleted_at.is_none())
            || self.reminders.iter().any(|r| r.deleted_at.is_none())
            || self.checkins.iter().any(|c| c.deleted_at.is_none())
            || self.memos.iter().any(|m| m.deleted_at.is_none());
        if !has_live {
            return;
        }

        let max_prev = self
            .courses
            .iter()
            .map(|e| e.sync_ts)
            .chain(self.sessions.iter().map(|e| e.sync_ts))
            .chain(self.events.iter().map(|e| e.sync_ts))
            .chain(self.reminders.iter().map(|e| e.sync_ts))
            .chain(self.checkins.iter().map(|e| e.sync_ts))
            .chain(self.memos.iter().map(|e| e.sync_ts))
            .max()
            .unwrap_or(0);
        let ts = self.next_stamp(max_prev);

        for c in self.courses.iter_mut().filter(|c| c.deleted_at.is_none()) {
            c.sync_ts = ts;
            c.deleted_at = Some(ts);
        }
        for s in self.sessions.iter_mut().filter(|s| s.deleted_at.is_none()) {
            s.sync_ts = ts;
            s.deleted_at = Some(ts);
        }
        for e in self.events.iter_mut().filter(|e| e.deleted_at.is_none()) {
            e.sync_ts = ts;
            e.deleted_at = Some(ts);
        }
        for r in self.reminders.iter_mut().filter(|r| r.deleted_at.is_none()) {
            r.sync_ts = ts;
            r.deleted_at = Some(ts);
        }
        for c in self.checkins.iter_mut().filter(|c| c.deleted_at.is_none()) {
            c.sync_ts = ts;
            c.deleted_at = Some(ts);
        }
        for m in self.memos.iter_mut().filter(|m| m.deleted_at.is_none()) {
            m.sync_ts = ts;
            m.deleted_at = Some(ts);
        }
    }

    /// 载入示例数据（替换语义）：先清空（盖墓碑），再插入一组全新实体（全部以当前戳落库）。
    pub fn load_sample_data(&mut self) {
        self.clear_all();
        let sample = crate::store::sample_data();
        for course in sample.courses {
            self.save_course(course);
        }
        for session in sample.sessions {
            self.save_session(session);
        }
        for event in sample.events {
            self.save_event(event);
        }
        for reminder in sample.reminders {
            self.save_reminder(reminder);
        }
        for checkin in sample.checkins {
            self.save_checkin(checkin);
        }
        for memo in sample.memos {
            self.save_memo(memo);
        }
        self.save_profile(sample.profile);
    }

    // MARK: - 同步面（协议 §2 / §7）

    /// 导出 `sync_ts >= since_ts` 的全部实体（**含墓碑**）为线协议 [`Change`]。
    ///
    /// `device_id` 留空由同步引擎填充；`payload` 为实体 JSON（墓碑亦携带，便于本地重建墓碑）。
    pub fn all_changes(&self, since_ts: i64) -> Vec<Change> {
        let mut out = Vec::new();
        let mut push = |kind: EntityKind,
                        id: &str,
                        sync_ts: i64,
                        deleted: bool,
                        payload: Option<serde_json::Value>| {
            if sync_ts < since_ts {
                return;
            }
            out.push(Change {
                kind,
                id: id.to_string(),
                updated_at: sync_ts,
                deleted,
                device_id: String::new(),
                payload,
                server_seq: None,
            });
        };

        push(
            EntityKind::Profile,
            "main",
            self.profile.sync_ts,
            false,
            serde_json::to_value(&self.profile).ok(),
        );
        for c in &self.courses {
            push(
                EntityKind::Course,
                &c.id,
                c.sync_ts,
                c.deleted_at.is_some(),
                serde_json::to_value(c).ok(),
            );
        }
        for s in &self.sessions {
            push(
                EntityKind::Session,
                &s.id,
                s.sync_ts,
                s.deleted_at.is_some(),
                serde_json::to_value(s).ok(),
            );
        }
        for e in &self.events {
            push(
                EntityKind::Event,
                &e.id,
                e.sync_ts,
                e.deleted_at.is_some(),
                serde_json::to_value(e).ok(),
            );
        }
        for r in &self.reminders {
            push(
                EntityKind::Reminder,
                &r.id,
                r.sync_ts,
                r.deleted_at.is_some(),
                serde_json::to_value(r).ok(),
            );
        }
        for c in &self.checkins {
            push(
                EntityKind::Checkin,
                &c.id,
                c.sync_ts,
                c.deleted_at.is_some(),
                serde_json::to_value(c).ok(),
            );
        }
        for m in &self.memos {
            push(
                EntityKind::Memo,
                &m.id,
                m.sync_ts,
                m.deleted_at.is_some(),
                serde_json::to_value(m).ok(),
            );
        }
        out
    }

    /// 全部实体标识（**含墓碑**，profile 记作 `("main")`）。
    ///
    /// 供首登「本地覆盖云端」策略计算「仅存在于服务端」的 id 之差
    /// （协议 §6-3c：`all_ids`（探针）减去 `all_entity_ids()`）。
    pub fn all_entity_ids(&self) -> Vec<(EntityKind, String)> {
        let mut out = Vec::new();
        out.push((EntityKind::Profile, "main".to_string()));
        out.extend(
            self.courses
                .iter()
                .map(|e| (EntityKind::Course, e.id.clone())),
        );
        out.extend(
            self.sessions
                .iter()
                .map(|e| (EntityKind::Session, e.id.clone())),
        );
        out.extend(
            self.events
                .iter()
                .map(|e| (EntityKind::Event, e.id.clone())),
        );
        out.extend(
            self.reminders
                .iter()
                .map(|e| (EntityKind::Reminder, e.id.clone())),
        );
        out.extend(
            self.checkins
                .iter()
                .map(|e| (EntityKind::Checkin, e.id.clone())),
        );
        out.extend(self.memos.iter().map(|e| (EntityKind::Memo, e.id.clone())));
        out
    }

    /// 首登「本地覆盖云端」重戳（协议 §6-3c，v1.2 R5）。
    ///
    /// 把**每一个**实体（含墓碑）的 `sync_ts` 抬到一个共享单调基址
    /// `base_ms + 递增序号` 之上（同时抬高 `stamp_floor_ms`，使后续本地改动
    /// 继续压过服务端已知戳）。`base_ms` 由 UI 取 `max(now_ms, server_time_ms + 1)`。
    ///
    /// 墓碑的 `deleted_at` 一并抬到新戳，保持「删除版本 = sync_ts」的不变量，
    /// 从而「本地覆盖云端」也覆盖云端较新的存活版本。
    ///
    /// 注意：这不是用户改动，不写入 `round_min_mut_ts`。
    pub fn bump_all_stamps(&mut self, base_ms: i64) {
        fn bump(ts: &mut i64, deleted_at: Option<&mut Option<i64>>, base_ms: i64, index: &mut i64) {
            let value = (*ts).max(base_ms.saturating_add(*index));
            *ts = value;
            if let Some(deleted) = deleted_at
                && deleted.is_some()
            {
                *deleted = Some(value);
            }
            *index = index.saturating_add(1);
        }

        let mut index: i64 = 0;
        bump(&mut self.profile.sync_ts, None, base_ms, &mut index);
        for e in self.courses.iter_mut() {
            bump(&mut e.sync_ts, Some(&mut e.deleted_at), base_ms, &mut index);
        }
        for e in self.sessions.iter_mut() {
            bump(&mut e.sync_ts, Some(&mut e.deleted_at), base_ms, &mut index);
        }
        for e in self.events.iter_mut() {
            bump(&mut e.sync_ts, Some(&mut e.deleted_at), base_ms, &mut index);
        }
        for e in self.reminders.iter_mut() {
            bump(&mut e.sync_ts, Some(&mut e.deleted_at), base_ms, &mut index);
        }
        for e in self.checkins.iter_mut() {
            bump(&mut e.sync_ts, Some(&mut e.deleted_at), base_ms, &mut index);
        }
        for e in self.memos.iter_mut() {
            bump(&mut e.sync_ts, Some(&mut e.deleted_at), base_ms, &mut index);
        }
        self.stamp_floor_ms = self.stamp_floor_ms.max(base_ms);
    }

    /// 统一 apply（协议 §2.4 + §7）：pull 批次与 push `overridden` 同走此函数。
    ///
    /// **不重戳**：落地的远端实体 `sync_ts = change.updated_at`；唯一例外是 checkin
    /// 合并 bump（协议 §7）。
    pub fn apply_remote(&mut self, change: &Change) -> ApplyOutcome {
        match change.kind {
            EntityKind::Profile => self.apply_remote_profile(change),
            EntityKind::Course => apply_lww(&mut self.courses, change),
            EntityKind::Session => apply_lww(&mut self.sessions, change),
            EntityKind::Event => apply_lww(&mut self.events, change),
            EntityKind::Reminder => apply_lww(&mut self.reminders, change),
            EntityKind::Checkin => self.apply_remote_checkin(change),
            EntityKind::Memo => apply_lww(&mut self.memos, change),
        }
    }

    fn apply_remote_profile(&mut self, change: &Change) -> ApplyOutcome {
        if change.deleted {
            // profile 为单例、无墓碑：忽略（防御性，服务端不会下发）。
            eprintln!("[sync] apply_remote: 忽略 profile 墓碑 id={}", change.id);
            return ApplyOutcome::KeptLocal;
        }
        let Some(payload) = &change.payload else {
            eprintln!("[sync] apply_remote: profile 实时变更缺少 payload，忽略");
            return ApplyOutcome::KeptLocal;
        };
        let mut remote = match serde_json::from_value::<Profile>(payload.clone()) {
            Ok(p) => p,
            Err(err) => {
                eprintln!("[sync] apply_remote: profile payload 解析失败: {err}");
                return ApplyOutcome::KeptLocal;
            }
        };
        if local_strictly_newer(self.profile.sync_ts, change.updated_at) {
            return ApplyOutcome::KeptLocal;
        }
        remote.sync_ts = change.updated_at;
        self.profile = remote;
        ApplyOutcome::Applied
    }

    /// checkin 统一 apply（协议 §7）：墓碑走常规路径；存活走 `history` map 合并。
    fn apply_remote_checkin(&mut self, change: &Change) -> ApplyOutcome {
        let ts = change.updated_at;
        if change.deleted {
            return apply_lww(&mut self.checkins, change);
        }
        let Some(payload) = &change.payload else {
            eprintln!(
                "[sync] apply_remote: checkin 实时变更缺少 payload，忽略 id={}",
                change.id
            );
            return ApplyOutcome::KeptLocal;
        };
        let remote = match serde_json::from_value::<CheckIn>(payload.clone()) {
            Ok(c) => c,
            Err(err) => {
                eprintln!("[sync] apply_remote: checkin payload 解析失败: {err}");
                return ApplyOutcome::KeptLocal;
            }
        };

        let Some(index) = self.checkins.iter().position(|c| c.id == change.id) else {
            // 本地不存在 → 远端胜，原样落地。
            let mut landed = remote;
            landed.sync_ts = ts;
            landed.deleted_at = None;
            self.checkins.push(landed);
            return ApplyOutcome::Applied;
        };

        let local_ts = self.checkins[index].sync_ts;
        let remote_wins = !local_strictly_newer(local_ts, ts);

        let mut merged = self.checkins[index].history.clone();
        for (key, value) in &remote.history {
            merged.insert(key.clone(), *value);
        }

        let mut out = if remote_wins {
            remote
        } else {
            self.checkins[index].clone()
        };

        if out.history != merged {
            out.history = merged;
            let bump = local_ts.max(ts).saturating_add(1);
            out.sync_ts = bump;
            out.deleted_at = None;
            self.checkins[index] = out;
            // 把合并 bump 折进本轮最小改动戳记账：本轮水位
            // `min(C0, server_time, round_min_mut_ts)` 因此 ≤ bump，
            // 合并后的并集不会被水位挡在下一轮之外（§5.3 R3 / §7）。
            self.round_min_mut_ts = Some(self.round_min_mut_ts.map_or(bump, |m| m.min(bump)));
            ApplyOutcome::MergedBumped
        } else {
            out.sync_ts = if remote_wins { ts } else { local_ts };
            out.deleted_at = None;
            self.checkins[index] = out;
            if remote_wins {
                ApplyOutcome::Applied
            } else {
                ApplyOutcome::KeptLocal
            }
        }
    }
}

// MARK: - 远端统一 apply 支撑（协议 §2.4 / §7）

/// 统一 apply 的结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApplyOutcome {
    /// 远端胜：落地远端版本（`sync_ts = change.updated_at`，不重戳）。
    Applied,
    /// 本地胜（本地严格更新）：保持本地不动，仍为下轮候选。
    KeptLocal,
    /// 仅 checkin：合并了 `history` 并 bump 戳，保持候选以便补推（协议 §7）。
    MergedBumped,
    /// 落地/刷新为本地墓碑。
    Tombstoned,
}

/// 可参与统一 apply 的实体：六类均有 `id` / `sync_ts` / `deleted_at`。
pub trait SyncEntity: Clone + Serialize + serde::de::DeserializeOwned {
    fn entity_id(&self) -> &str;
    fn sync_ts(&self) -> i64;
    fn deleted_at(&self) -> Option<i64>;
    fn set_sync_ts(&mut self, ts: i64);
    fn set_deleted_at(&mut self, value: Option<i64>);
}

macro_rules! impl_sync_entity {
    ($t:ty) => {
        impl SyncEntity for $t {
            fn entity_id(&self) -> &str {
                &self.id
            }
            fn sync_ts(&self) -> i64 {
                self.sync_ts
            }
            fn deleted_at(&self) -> Option<i64> {
                self.deleted_at
            }
            fn set_sync_ts(&mut self, ts: i64) {
                self.sync_ts = ts;
            }
            fn set_deleted_at(&mut self, value: Option<i64>) {
                self.deleted_at = value;
            }
        }
    };
}

impl_sync_entity!(Course);
impl_sync_entity!(Session);
impl_sync_entity!(EventItem);
impl_sync_entity!(Reminder);
impl_sync_entity!(CheckIn);
impl_sync_entity!(Memo);

/// 本地版本是否**严格**更新。仅当本地严格更新时保持本地；平局让位远端
/// （服务端持有者胜，协议 §2.2 / §2.4「只有比远端更新的本地编辑受保护」）。
fn local_strictly_newer(local_ts: i64, incoming_ts: i64) -> bool {
    wins(local_ts, incoming_ts)
}

/// profile/course/session/event/reminder/memo 的常规统一 apply（协议 §2.4）。
fn apply_lww<T: SyncEntity>(store: &mut Vec<T>, change: &Change) -> ApplyOutcome {
    let ts = change.updated_at;
    let index = store.iter().position(|e| e.entity_id() == change.id);

    if change.deleted {
        return match index {
            Some(i) => {
                if local_strictly_newer(store[i].sync_ts(), ts) {
                    ApplyOutcome::KeptLocal
                } else {
                    store[i].set_sync_ts(ts);
                    store[i].set_deleted_at(Some(ts));
                    ApplyOutcome::Tombstoned
                }
            }
            None => {
                // 本地没有该实体：远端墓碑带 payload 时落一条本地墓碑以固化删除；
                // 否则视为无操作（没有可删除的数据）。
                match change
                    .payload
                    .as_ref()
                    .and_then(|value| serde_json::from_value::<T>(value.clone()).ok())
                {
                    Some(mut entity) => {
                        entity.set_sync_ts(ts);
                        entity.set_deleted_at(Some(ts));
                        store.push(entity);
                        ApplyOutcome::Tombstoned
                    }
                    None => ApplyOutcome::KeptLocal,
                }
            }
        };
    }

    let Some(payload) = &change.payload else {
        eprintln!(
            "[sync] apply_remote: 实时变更缺少 payload，忽略 kind={} id={}",
            change.kind.as_str(),
            change.id
        );
        return ApplyOutcome::KeptLocal;
    };
    let mut remote = match serde_json::from_value::<T>(payload.clone()) {
        Ok(entity) => entity,
        Err(err) => {
            eprintln!(
                "[sync] apply_remote: payload 解析失败 kind={} id={}: {err}",
                change.kind.as_str(),
                change.id
            );
            return ApplyOutcome::KeptLocal;
        }
    };

    match index {
        Some(i) if local_strictly_newer(store[i].sync_ts(), ts) => ApplyOutcome::KeptLocal,
        Some(i) => {
            remote.set_sync_ts(ts);
            remote.set_deleted_at(None);
            store[i] = remote;
            ApplyOutcome::Applied
        }
        None => {
            remote.set_sync_ts(ts);
            remote.set_deleted_at(None);
            store.push(remote);
            ApplyOutcome::Applied
        }
    }
}

// MARK: - Helpers

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

pub fn now_minutes() -> i32 {
    let t = Local::now().time();
    time_minutes(t)
}

#[cfg(test)]
thread_local! {
    /// 测试专用确定性时钟：`Some(v)` 时 [`now_ms`] 固定返回 `v`。
    static TEST_NOW_MS: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

/// 当前 unix 毫秒（本地时钟），所有本地写戳的基准。
pub fn now_ms() -> i64 {
    #[cfg(test)]
    {
        if let Some(value) = TEST_NOW_MS.with(|cell| cell.get()) {
            return value;
        }
    }
    now_millis() as i64
}

/// 测试专用：注入确定性的「当前毫秒」（`None` 恢复真实时钟）。
///
/// 采用 thread-local 覆盖：每个 `#[test]` 运行在独立线程，互不干扰，无需全局锁。
#[cfg(test)]
pub fn set_test_now_ms(ms: Option<i64>) {
    TEST_NOW_MS.with(|cell| cell.set(ms));
}

/// 周一为 1，周日为 7。
pub fn iso_weekday(date: NaiveDate) -> u8 {
    match date.weekday() {
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
        Weekday::Sun => 7,
    }
}

pub fn date_of(weekday: u8) -> NaiveDate {
    let wd = weekday.clamp(1, 7);
    let today = today();
    let current = iso_weekday(today);
    today + Duration::days(wd as i64 - current as i64)
}

pub fn fmt_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn fmt_date_cn(date: NaiveDate) -> String {
    format!(
        "{}月{}日 {}",
        date.month(),
        date.day(),
        WEEKDAY_LABELS[(iso_weekday(date) - 1) as usize]
    )
}

pub fn relative_day(date: NaiveDate) -> String {
    let t = today();
    if date == t {
        "今天".to_string()
    } else if date == t - Duration::days(1) {
        "昨天".to_string()
    } else if date == t + Duration::days(1) {
        "明天".to_string()
    } else if date == t + Duration::days(2) {
        "后天".to_string()
    } else {
        format!("{}月{}日", date.month(), date.day())
    }
}

pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{:x}", now_millis(), n)
}

fn now_millis() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn course(id: &str, name: &str) -> Course {
        let mut c = Course::new(name, CourseKind::School);
        c.id = id.into();
        c
    }

    fn live_change<T: Serialize>(kind: EntityKind, id: &str, ts: i64, entity: &T) -> Change {
        Change {
            kind,
            id: id.into(),
            updated_at: ts,
            deleted: false,
            device_id: "test".into(),
            payload: Some(serde_json::to_value(entity).unwrap()),
            server_seq: None,
        }
    }

    fn tombstone_change(kind: EntityKind, id: &str, ts: i64) -> Change {
        Change {
            kind,
            id: id.into(),
            updated_at: ts,
            deleted: true,
            device_id: "test".into(),
            payload: None,
            server_seq: None,
        }
    }

    fn stored_course<'a>(db: &'a Database, id: &str) -> &'a Course {
        db.courses.iter().find(|c| c.id == id).unwrap()
    }

    // MARK: 戳规则

    #[test]
    fn stamp_is_monotonic_across_clock_rollback() {
        set_test_now_ms(Some(1_000_000));
        let mut db = Database::default();

        db.save_course(course("c1", "数学"));
        assert_eq!(stored_course(&db, "c1").sync_ts, 1_000_000);

        // 时钟回拨：仍必须严格压过本地基线（prev + 1）。
        set_test_now_ms(Some(500_000));
        let mut edited = stored_course(&db, "c1").clone();
        edited.name = "数学（改）".into();
        db.save_course(edited);
        assert_eq!(stored_course(&db, "c1").sync_ts, 1_000_001);

        // 水位下限：即便时钟持续回拨，也保证 ts >= watermark + 1。
        db.stamp_floor_ms = 2_000_000;
        let mut edited = stored_course(&db, "c1").clone();
        edited.name = "数学（再改）".into();
        db.save_course(edited);
        assert_eq!(stored_course(&db, "c1").sync_ts, 2_000_001);

        set_test_now_ms(None);
    }

    #[test]
    fn round_min_mutation_ts_tracks_and_resets() {
        set_test_now_ms(Some(10_000));
        let mut db = Database::default();
        db.begin_round(0);

        db.save_course(course("c-a", "A"));
        set_test_now_ms(Some(20_000));
        db.save_course(course("c-b", "B"));
        assert_eq!(db.round_min_mutation_ts(), Some(10_000));

        db.begin_round(0);
        assert_eq!(db.round_min_mutation_ts(), None);

        set_test_now_ms(None);
    }

    // MARK: 墓碑与读取过滤

    #[test]
    fn tombstone_filtering_hides_deleted_entities() {
        let mut db = Database::default();
        db.save_course(course("c1", "数学"));
        db.save_session(Session::new("c1", 1, "08:00", "08:45", "A-302"));
        let session_id = db.sessions[0].id.clone();

        assert_eq!(db.live_course_count(), 1);
        assert!(db.course("c1").is_some());
        assert_eq!(db.sessions_on(1).len(), 1);

        db.delete_session(&session_id);
        assert_eq!(db.live_session_count(), 0);
        assert!(db.sessions_on(1).is_empty());
        // 物理数据保留（墓碑），不真正删除。
        assert_eq!(db.sessions.len(), 1);
        assert!(db.sessions[0].deleted_at.is_some());
    }

    #[test]
    fn delete_course_cascades_sessions_with_same_stamp() {
        set_test_now_ms(Some(100_000));
        let mut db = Database::default();
        db.save_course(course("c1", "数学"));
        db.save_course(course("c2", "语文"));
        set_test_now_ms(Some(200_000));
        db.save_session(Session::new("c1", 1, "08:00", "08:45", "A-302"));
        db.save_session(Session::new("c2", 2, "09:00", "09:45", "B-105"));
        let s1 = db
            .sessions
            .iter()
            .find(|s| s.course_id == "c1")
            .unwrap()
            .id
            .clone();

        set_test_now_ms(Some(300_000));
        db.delete_course("c1");

        let course_ts = stored_course(&db, "c1").deleted_at.unwrap();
        let s1_ts = db
            .sessions
            .iter()
            .find(|s| s.id == s1)
            .unwrap()
            .deleted_at
            .unwrap();
        assert_eq!(course_ts, s1_ts, "级联课表安排应与课程共用一个墓碑戳");
        // 另一门课程的安排不受影响。
        assert!(
            db.sessions
                .iter()
                .find(|s| s.course_id == "c2")
                .unwrap()
                .deleted_at
                .is_none()
        );
        assert_eq!(db.live_session_count(), 1);

        set_test_now_ms(None);
    }

    #[test]
    fn clear_all_tombstones_everything_but_keeps_profile() {
        let mut db = Database::default();
        db.profile.student = "小满".into();
        db.save_course(course("c1", "数学"));
        db.save_session(Session::new("c1", 1, "08:00", "08:45", "A-302"));
        db.save_memo(Memo::new());

        db.clear_all();

        assert_eq!(db.live_course_count(), 0);
        assert_eq!(db.live_session_count(), 0);
        assert_eq!(db.live_memo_count(), 0);
        // 物理数据保留。
        assert!(!db.courses.is_empty());
        // profile 无墓碑，保留。
        assert_eq!(db.profile.student, "小满");

        // 所有墓碑共享同一戳。
        let mut stamps: Vec<i64> = db.courses.iter().filter_map(|e| e.deleted_at).collect();
        stamps.extend(db.sessions.iter().filter_map(|e| e.deleted_at));
        stamps.extend(db.memos.iter().filter_map(|e| e.deleted_at));
        assert!(!stamps.is_empty());
        assert!(stamps.iter().all(|s| *s == stamps[0]));
    }

    #[test]
    fn load_sample_data_replaces_existing_content() {
        let mut db = Database::default();
        db.save_course(course("stale", "旧课程"));
        assert!(db.course("stale").is_some());

        db.load_sample_data();

        assert!(db.course("stale").is_none());
        assert!(db.live_course_count() > 0);
        assert!(db.live_session_count() > 0);
        // 旧课程成为墓碑（替换语义 = 清空 + 新增）。
        assert!(
            db.courses
                .iter()
                .any(|c| c.id == "stale" && c.deleted_at.is_some())
        );
    }

    // MARK: apply_remote（§2.4）

    #[test]
    fn apply_remote_win_lose_tie() {
        // 胜：远端严格更新 → 落地。
        let mut db = Database::default();
        db.save_course(course("c1", "本地"));
        let local_ts = stored_course(&db, "c1").sync_ts;
        let remote = course("c1", "远端");
        let change = live_change(EntityKind::Course, "c1", local_ts + 10, &remote);
        assert_eq!(db.apply_remote(&change), ApplyOutcome::Applied);
        assert_eq!(stored_course(&db, "c1").name, "远端");
        assert_eq!(stored_course(&db, "c1").sync_ts, local_ts + 10);

        // 负：本地严格更新 → 保持本地。
        let mut db = Database::default();
        db.save_course(course("c1", "本地"));
        let local_ts = stored_course(&db, "c1").sync_ts;
        let remote = course("c1", "远端");
        let change = live_change(EntityKind::Course, "c1", local_ts - 10, &remote);
        assert_eq!(db.apply_remote(&change), ApplyOutcome::KeptLocal);
        assert_eq!(stored_course(&db, "c1").name, "本地");

        // 平局：让位远端（服务端持有者胜）。
        let mut db = Database::default();
        db.save_course(course("c1", "本地"));
        let local_ts = stored_course(&db, "c1").sync_ts;
        let remote = course("c1", "远端");
        let change = live_change(EntityKind::Course, "c1", local_ts, &remote);
        assert_eq!(db.apply_remote(&change), ApplyOutcome::Applied);
        assert_eq!(stored_course(&db, "c1").name, "远端");
    }

    #[test]
    fn apply_remote_missing_local_applies() {
        let mut db = Database::default();
        let remote = course("new", "新课程");
        let change = live_change(EntityKind::Course, "new", 777, &remote);
        assert_eq!(db.apply_remote(&change), ApplyOutcome::Applied);
        assert_eq!(stored_course(&db, "new").sync_ts, 777);
    }

    #[test]
    fn apply_remote_tombstone_win_lose() {
        // 本地较旧 → 落地墓碑。
        let mut db = Database::default();
        db.save_course(course("c1", "数学"));
        let local_ts = stored_course(&db, "c1").sync_ts;
        let change = tombstone_change(EntityKind::Course, "c1", local_ts + 5);
        assert_eq!(db.apply_remote(&change), ApplyOutcome::Tombstoned);
        assert!(db.course("c1").is_none());
        assert_eq!(stored_course(&db, "c1").deleted_at, Some(local_ts + 5));

        // 本地较新 → 忽略墓碑，保持本地。
        let mut db = Database::default();
        db.save_course(course("c1", "数学"));
        let local_ts = stored_course(&db, "c1").sync_ts;
        let change = tombstone_change(EntityKind::Course, "c1", local_ts - 5);
        assert_eq!(db.apply_remote(&change), ApplyOutcome::KeptLocal);
        assert!(db.course("c1").is_some());
    }

    #[test]
    fn apply_remote_unknown_or_invalid_payload_is_kept_local() {
        let mut db = Database::default();
        db.save_course(course("c1", "数学"));
        let bad = Change {
            kind: EntityKind::Course,
            id: "c1".into(),
            updated_at: i64::MAX,
            deleted: false,
            device_id: String::new(),
            payload: Some(serde_json::json!({ "not": "a course" })),
            server_seq: None,
        };
        assert_eq!(db.apply_remote(&bad), ApplyOutcome::KeptLocal);

        let null_live = Change {
            kind: EntityKind::Course,
            id: "c1".into(),
            updated_at: i64::MAX,
            deleted: false,
            device_id: String::new(),
            payload: None,
            server_seq: None,
        };
        assert_eq!(db.apply_remote(&null_live), ApplyOutcome::KeptLocal);
    }

    // MARK: checkin 合并（§7）

    #[test]
    fn checkin_merge_bumps_when_local_wins_with_extra_remote_keys() {
        let mut db = Database::default();
        let mut local = CheckIn::new("阅读", Repeat::Daily);
        local.id = "ci".into();
        local.history.insert("2026-10-01".into(), true);
        local.sync_ts = 100;
        db.checkins.push(local);

        let mut remote = CheckIn::new("阅读", Repeat::Daily);
        remote.id = "ci".into();
        remote.history.insert("2026-10-02".into(), false);
        let change = live_change(EntityKind::Checkin, "ci", 50, &remote);

        assert_eq!(db.apply_remote(&change), ApplyOutcome::MergedBumped);
        let merged = &db.checkins[0];
        assert_eq!(merged.history.get("2026-10-01"), Some(&true));
        assert_eq!(merged.history.get("2026-10-02"), Some(&false));
        // bump 严格高于双方，保持候选。
        assert_eq!(merged.sync_ts, 101);
    }

    #[test]
    fn checkin_merge_bump_feeds_round_min_mutation_ts() {
        set_test_now_ms(Some(10_000));
        let mut db = Database::default();
        db.begin_round(0);
        let mut local = CheckIn::new("阅读", Repeat::Daily);
        local.id = "ci".into();
        local.history.insert("2026-10-01".into(), true);
        local.sync_ts = 100;
        db.checkins.push(local);

        let mut remote = CheckIn::new("阅读", Repeat::Daily);
        remote.id = "ci".into();
        remote.history.insert("2026-10-02".into(), false);
        let change = live_change(EntityKind::Checkin, "ci", 50, &remote);

        assert_eq!(db.apply_remote(&change), ApplyOutcome::MergedBumped);
        let bump = db.checkins[0].sync_ts;
        assert_eq!(bump, 101);
        // 合并 bump 必须折进本轮最小改动戳：否则水位会把它挡在下一轮之外。
        assert_eq!(db.round_min_mutation_ts(), Some(bump));

        // 再来一次合并：bump 更大，但 round_min 取本轮更小者（保持 ≤ 任一次 bump）。
        let mut remote2 = CheckIn::new("阅读", Repeat::Daily);
        remote2.id = "ci".into();
        remote2.history.insert("2026-10-03".into(), true);
        let change2 = live_change(EntityKind::Checkin, "ci", 102, &remote2);
        assert_eq!(db.apply_remote(&change2), ApplyOutcome::MergedBumped);
        assert_eq!(db.checkins[0].sync_ts, 103);
        assert_eq!(db.round_min_mutation_ts(), Some(101));

        // 新一轮：round_min 归零后，单次合并 bump 重新成为下限。
        db.begin_round(0);
        assert_eq!(db.round_min_mutation_ts(), None);
        let mut remote3 = CheckIn::new("阅读", Repeat::Daily);
        remote3.id = "ci".into();
        remote3.history.insert("2026-10-04".into(), true);
        let change3 = live_change(EntityKind::Checkin, "ci", 50, &remote3);
        assert_eq!(db.apply_remote(&change3), ApplyOutcome::MergedBumped);
        assert_eq!(db.checkins[0].sync_ts, 104);
        assert_eq!(db.round_min_mutation_ts(), Some(104));

        set_test_now_ms(None);
    }

    #[test]
    fn checkin_merge_lands_winner_without_bump_when_subset() {
        let mut db = Database::default();
        let mut local = CheckIn::new("阅读", Repeat::Daily);
        local.id = "ci".into();
        local.history.insert("2026-10-01".into(), true);
        local.history.insert("2026-10-02".into(), true);
        local.sync_ts = 100;
        db.checkins.push(local);

        let mut remote = CheckIn::new("阅读", Repeat::Daily);
        remote.id = "ci".into();
        remote.history.insert("2026-10-02".into(), true); // 本地已含，无新增
        let change = live_change(EntityKind::Checkin, "ci", 50, &remote);

        assert_eq!(db.apply_remote(&change), ApplyOutcome::KeptLocal);
        assert_eq!(db.checkins[0].sync_ts, 100);
    }

    #[test]
    fn checkin_merge_converges_across_devices() {
        let mut a = Database::default();
        let mut ca = CheckIn::new("阅读", Repeat::Daily);
        ca.id = "ci".into();
        ca.history.insert("2026-10-01".into(), true);
        ca.sync_ts = 100;
        a.checkins.push(ca);

        let mut b = Database::default();
        let mut cb = CheckIn::new("阅读", Repeat::Daily);
        cb.id = "ci".into();
        cb.history.insert("2026-10-02".into(), true);
        cb.sync_ts = 100;
        b.checkins.push(cb);

        for _ in 0..4 {
            for change in a.all_changes(0) {
                b.apply_remote(&change);
            }
            for change in b.all_changes(0) {
                a.apply_remote(&change);
            }
        }

        let ha = a.checkins.iter().find(|c| c.id == "ci").unwrap();
        let hb = b.checkins.iter().find(|c| c.id == "ci").unwrap();
        assert_eq!(ha.history, hb.history);
        assert_eq!(ha.history.get("2026-10-01"), Some(&true));
        assert_eq!(ha.history.get("2026-10-02"), Some(&true));

        // 再交换一轮，状态应稳定（收敛）。
        let before_ts = ha.sync_ts;
        for change in a.all_changes(0) {
            b.apply_remote(&change);
        }
        for change in b.all_changes(0) {
            a.apply_remote(&change);
        }
        let ha = a.checkins.iter().find(|c| c.id == "ci").unwrap();
        let hb = b.checkins.iter().find(|c| c.id == "ci").unwrap();
        assert_eq!(ha.sync_ts, before_ts);
        assert_eq!(ha.sync_ts, hb.sync_ts);
        assert_eq!(ha.history, hb.history);
    }

    // MARK: all_changes

    #[test]
    fn all_changes_payload_round_trips_and_includes_tombstones() {
        let mut db = Database::default();
        db.save_course(course("c1", "数学"));
        let memo = Memo::new();
        let memo_id = memo.id.clone();
        db.save_memo(memo);
        db.delete_memo(&memo_id);

        let course_ts = stored_course(&db, "c1").sync_ts;
        let changes = db.all_changes(0);

        let course_change = changes
            .iter()
            .find(|c| c.kind == EntityKind::Course && c.id == "c1")
            .expect("应包含课程变更");
        assert!(!course_change.deleted);
        assert_eq!(course_change.updated_at, course_ts);
        assert!(course_change.device_id.is_empty());
        assert!(course_change.server_seq.is_none());
        let decoded: Course =
            serde_json::from_value(course_change.payload.clone().unwrap()).unwrap();
        assert_eq!(decoded.name, "数学");
        assert_eq!(decoded.sync_ts, course_ts);

        let memo_change = changes
            .iter()
            .find(|c| c.kind == EntityKind::Memo && c.id == memo_id)
            .expect("应包含备忘墓碑");
        assert!(memo_change.deleted);
        let decoded: Memo = serde_json::from_value(memo_change.payload.clone().unwrap()).unwrap();
        assert!(decoded.deleted_at.is_some());

        // since_ts 过滤。
        let later = db.all_changes(course_ts + 1);
        assert!(
            later
                .iter()
                .all(|c| c.updated_at > course_ts && c.kind != EntityKind::Course)
        );

        // profile 出现在 kind=profile / id=main。
        let profile_change = changes
            .iter()
            .find(|c| c.kind == EntityKind::Profile)
            .unwrap();
        assert_eq!(profile_change.id, "main");
        assert!(!profile_change.deleted);
    }

    #[test]
    fn save_memo_refreshes_display_and_keeps_created_at() {
        let mut db = Database::default();
        let mut memo = Memo::new();
        memo.id = "m1".into();
        memo.created_at = "2020-01-01 00:00".into();
        memo.updated_at = "2020-01-01 00:00".into();
        db.save_memo(memo);

        let mut edited = db.memos.iter().find(|m| m.id == "m1").unwrap().clone();
        edited.title = "改".into();
        db.save_memo(edited);

        let stored = db.memos.iter().find(|m| m.id == "m1").unwrap();
        assert_eq!(stored.created_at, "2020-01-01 00:00");
        assert_ne!(stored.updated_at, "2020-01-01 00:00");
    }
}
