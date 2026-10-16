use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, Timelike, Weekday};
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
}

impl Session {
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
}

/// 日程提醒。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub content: String,
    /// "2026-03-05 08:00"
    pub due_at: String,
    pub repeat: Repeat,
    pub done: bool,
    pub notified_for: String,
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
    pub updated_at: String,
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
}

impl Database {
    pub fn course(&self, id: &str) -> Option<&Course> {
        self.courses.iter().find(|c| c.id == id)
    }

    pub fn course_name(&self, id: &str) -> String {
        self.course(id).map(|c| c.name.clone()).unwrap_or_default()
    }

    /// 指定星期几的课程安排，按开始时间排序。
    pub fn sessions_on(&self, weekday: u8) -> Vec<&Session> {
        let mut list: Vec<&Session> = self
            .sessions
            .iter()
            .filter(|s| s.weekday == weekday)
            .collect();
        list.sort_by_key(|s| (s.start_minutes(), s.end_minutes()));
        list
    }

    /// 一周的总课时（分钟）。
    pub fn weekly_minutes(&self) -> i64 {
        self.sessions
            .iter()
            .map(|s| (s.end_minutes() - s.start_minutes()).max(0) as i64)
            .sum()
    }

    pub fn sessions_of(&self, course_id: &str) -> Vec<&Session> {
        let mut list: Vec<&Session> = self
            .sessions
            .iter()
            .filter(|s| s.course_id == course_id)
            .collect();
        list.sort_by_key(|s| (s.weekday, s.start_minutes()));
        list
    }

    pub fn reminders_on(&self, date: NaiveDate) -> Vec<&Reminder> {
        let key = date.format("%Y-%m-%d").to_string();
        let mut list: Vec<&Reminder> = self
            .reminders
            .iter()
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
        let mut list: Vec<&EventItem> = self.events.iter().filter(|e| e.date == key).collect();
        list.sort_by_key(|e| e.start.clone());
        list
    }

    pub fn checkins_on(&self, date: NaiveDate) -> Vec<&CheckIn> {
        self.checkins.iter().filter(|c| c.active_on(date)).collect()
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
