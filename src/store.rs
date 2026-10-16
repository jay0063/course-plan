use crate::model::{Database, now_ms};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 数据目录。测试会向其中注入确定性数据；正常运行时存放 `data.json` /
/// `sync.json` / `runtime_state.json`。
pub fn data_dir() -> PathBuf {
    dirs().join("course-plan")
}

fn dirs() -> PathBuf {
    if let Some(dir) = directories::ProjectDirs::from("com", "courseplan", "CoursePlan") {
        return dir.data_dir().to_path_buf();
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
}

pub fn data_file() -> PathBuf {
    data_dir().join("data.json")
}

/// 同步凭据 / 游标文件（含 token，权限 0600）。
pub fn sync_file() -> PathBuf {
    data_dir().join("sync.json")
}

/// 设备本地运行态文件（提醒「已通知」记账等，不进同步）。
pub fn runtime_state_file() -> PathBuf {
    data_dir().join("runtime_state.json")
}

/// 读取数据库。
///
/// - 文件不存在 → `(Database::default(), None)`（**不再自动播种示例数据**）。
/// - 文件无法解析 → 原文件备份为 `data.json.corrupt-<unix_ms>` 后返回空库与警告文案
///   （绝不静默覆盖）。
/// - 解析成功 → 迁移旧 `Reminder.notified_for` 进 `runtime_state.json` 后返回。
pub fn load() -> (Database, Option<String>) {
    let path = data_file();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return (Database::default(), None),
    };

    match serde_json::from_str::<Database>(&text) {
        Ok(db) => {
            let mut runtime = load_runtime_state();
            if migrate_notified_for(&text, &mut runtime) {
                let _ = save_runtime_state(&runtime);
            }
            (db, None)
        }
        Err(_) => {
            let backup = path.with_file_name(format!("data.json.corrupt-{}", now_ms()));
            let _ = fs::rename(&path, &backup);
            (
                Database::default(),
                Some(format!(
                    "数据文件无法解析，已备份为 {}，本次以空数据启动（未覆盖原文件）。",
                    backup.display()
                )),
            )
        }
    }
}

pub fn save(db: &Database) -> Result<(), String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let path = data_file();
    let text = serde_json::to_string_pretty(db).map_err(|e| format!("序列化失败: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| format!("写入失败: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("保存失败: {e}"))?;
    Ok(())
}

// MARK: - sync.json（同步凭据，0600）

/// `sync.json` v2：服务端地址 + 账号凭据 + 设备标识 + 游标/水位。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SyncSettings {
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub last_seq: u64,
    #[serde(default)]
    pub push_watermark_ms: i64,
}

/// 读取 `sync.json`（缺失 / 损坏 → 默认值，不覆盖文件）。
pub fn load_sync() -> SyncSettings {
    match fs::read_to_string(sync_file()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => SyncSettings::default(),
    }
}

/// 写入 `sync.json`：首次保存自动生成 `device_id`；unix 上以 0600 权限创建。
pub fn save_sync(settings: &mut SyncSettings) -> Result<(), String> {
    if settings.device_id.trim().is_empty() {
        settings.device_id = generate_device_id();
    }
    let text = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化 sync.json 失败: {e}"))?;
    write_private(&sync_file(), &text)
}

/// 生成设备标识：沿用 `new_id()` 的时间 + 原子计数样式，再叠加一个独立 token。
fn generate_device_id() -> String {
    format!("dev-{}-{}", crate::model::new_id(), crate::model::new_id())
}

// MARK: - runtime_state.json（设备本地运行态，不同步）

/// 设备本地运行态。目前仅存提醒「已通知」记账：`reminder_id -> 日期键`。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeState {
    #[serde(default)]
    pub notified: BTreeMap<String, String>,
}

/// 读取运行态（缺失 / 损坏 → 默认值）。
pub fn load_runtime_state() -> RuntimeState {
    match fs::read_to_string(runtime_state_file()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => RuntimeState::default(),
    }
}

pub fn save_runtime_state(state: &RuntimeState) -> Result<(), String> {
    let text = serde_json::to_string_pretty(state).map_err(|e| format!("序列化运行态失败: {e}"))?;
    atomic_write(&runtime_state_file(), &text)
}

/// 把旧档案里 `Reminder.notified_for` 的非空值迁移进运行态；返回是否有改动。
///
/// 单独抽出便于单元测试（不触碰磁盘）。
pub fn migrate_notified_for(raw: &str, runtime: &mut RuntimeState) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return false;
    };
    let Some(reminders) = value.get("reminders").and_then(|v| v.as_array()) else {
        return false;
    };
    let mut changed = false;
    for reminder in reminders {
        let Some(id) = reminder.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let notified = reminder
            .get("notified_for")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !notified.is_empty() && runtime.notified.get(id).map(String::as_str) != Some(notified) {
            runtime
                .notified
                .insert(id.to_string(), notified.to_string());
            changed = true;
        }
    }
    changed
}

// MARK: - 原子写

fn atomic_write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建数据目录失败: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text).map_err(|e| format!("写入失败: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("保存失败: {e}"))?;
    Ok(())
}

/// 同 [`atomic_write`]，但 unix 上以 0600 权限创建（凭据文件）。
fn write_private(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建数据目录失败: {e}"))?;
    }
    let tmp = path.with_extension("tmp");

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| format!("写入失败: {e}"))?;
        file.write_all(text.as_bytes())
            .map_err(|e| format!("写入失败: {e}"))?;
        let _ = file.sync_all();
    }
    #[cfg(not(unix))]
    {
        fs::write(&tmp, text).map_err(|e| format!("写入失败: {e}"))?;
    }

    fs::rename(&tmp, path).map_err(|e| format!("保存失败: {e}"))?;
    Ok(())
}

/// 构造一份示例数据：一门学校课程 + 一门补习课程、一周课表、需要完成的提醒与打卡、两条备忘。
///
/// 供「载入示例数据」动作与无头渲染测试（`tests/render.rs`）注入确定性的数据使用。
pub fn sample_data() -> Database {
    use crate::model::*;

    let math = Course::new("数学", CourseKind::School);
    let chinese = Course::new("语文", CourseKind::School);
    let english = Course::new("英语", CourseKind::School);
    let gym = Course::new("体育", CourseKind::School);
    let art = Course::new("美术", CourseKind::School);
    let piano = Course::new("钢琴", CourseKind::Tutoring);
    let olympic = Course::new("奥数", CourseKind::Tutoring);

    let math_id = math.id.clone();
    let chinese_id = chinese.id.clone();
    let english_id = english.id.clone();
    let gym_id = gym.id.clone();
    let art_id = art.id.clone();
    let piano_id = piano.id.clone();
    let olympic_id = olympic.id.clone();

    let mut math_c = math;
    math_c.subject = "数学".into();
    math_c.teacher = "王老师".into();
    math_c.location = "教学楼 A-302".into();
    math_c.color = 0;
    let mut chinese_c = chinese;
    chinese_c.subject = "语文".into();
    chinese_c.teacher = "李老师".into();
    chinese_c.location = "教学楼 A-201".into();
    chinese_c.color = 1;
    let mut english_c = english;
    english_c.subject = "英语".into();
    english_c.teacher = "Miss Chen".into();
    english_c.location = "教学楼 B-105".into();
    english_c.color = 2;
    let mut gym_c = gym;
    gym_c.subject = "体育".into();
    gym_c.location = "操场".into();
    gym_c.color = 3;
    let mut art_c = art;
    art_c.subject = "美术".into();
    art_c.location = "美术楼 1F".into();
    art_c.color = 4;
    let mut piano_c = piano;
    piano_c.subject = "音乐".into();
    piano_c.teacher = "周老师".into();
    piano_c.location = "琴房 3F".into();
    piano_c.note = "每周六上午，需自带乐谱".into();
    piano_c.color = 5;
    let mut olympic_c = olympic;
    olympic_c.subject = "数学".into();
    olympic_c.teacher = "陈老师".into();
    olympic_c.location = "培训中心 201".into();
    olympic_c.note = "思维拓展专题".into();
    olympic_c.color = 6;

    let sessions = vec![
        Session::new(&chinese_id, 1, "08:00", "08:45", "A-201"),
        Session::new(&math_id, 1, "08:55", "09:40", "A-302"),
        Session::new(&english_id, 1, "10:00", "10:45", "B-105"),
        Session::new(&gym_id, 1, "14:00", "14:45", "操场"),
        Session::new(&math_id, 2, "08:00", "08:45", "A-302"),
        Session::new(&english_id, 2, "08:55", "09:40", "B-105"),
        Session::new(&chinese_id, 2, "10:00", "10:45", "A-201"),
        Session::new(&math_id, 3, "08:00", "08:45", "A-302"),
        Session::new(&art_id, 3, "09:50", "10:35", "美术楼"),
        Session::new(&chinese_id, 3, "14:00", "14:45", "A-201"),
        Session::new(&math_id, 4, "08:00", "08:45", "A-302"),
        Session::new(&english_id, 4, "08:55", "09:40", "B-105"),
        Session::new(&chinese_id, 5, "08:00", "08:45", "A-201"),
        Session::new(&art_id, 5, "10:00", "10:45", "美术楼"),
        Session::new(&piano_id, 6, "09:00", "10:00", "琴房 3F"),
        Session::new(&olympic_id, 6, "10:20", "11:50", "培训中心 201"),
        Session::new(&olympic_id, 7, "09:30", "11:00", "培训中心 201"),
    ];

    let mut read20 = CheckIn::new("课外阅读 20 分钟", Repeat::Daily);
    read20.note = "阅读《安徒生童话》，记录好词好句".into();
    read20.created_at = now_string();

    let mut recite = CheckIn::new("背诵古诗一首", Repeat::Daily);
    recite.created_at = now_string();

    let mut practice = CheckIn::new("口算练习", Repeat::Weekly);
    practice.weekdays = vec![1, 2, 3, 4, 5];
    practice.note = "限时 5 分钟，20 道题".into();
    practice.created_at = now_string();

    let mut handwriting = CheckIn::new("练字一页", Repeat::Weekly);
    handwriting.weekdays = vec![1, 3, 5];
    handwriting.created_at = now_string();

    let mut homework = Reminder::new("提交数学作业", format!("{} 20:00", today()), Repeat::Daily);
    homework.content = "练习册 P32-33".into();

    let memo = {
        let mut m = Memo::new();
        m.title = "期中考试范围".into();
        m.content = "语文：第 1-6 课文背诵\n数学：分数加减法应用题\n英语：Unit 1-3 单词".into();
        m.color = 0;
        m.pinned = true;
        m
    };

    let memo2 = {
        let mut m = Memo::new();
        m.title = "家长会要点".into();
        m.content = "1. 期中成绩分析\n2. 周末作业量调整\n3. 体育测试安排".into();
        m.color = 1;
        m
    };

    Database {
        profile: Profile {
            student: "小满".into(),
            grade: "三年级".into(),
            school: "阳光第一小学".into(),
            ..Profile::default()
        },
        courses: vec![
            math_c, chinese_c, english_c, gym_c, art_c, piano_c, olympic_c,
        ],
        sessions,
        events: Vec::new(),
        reminders: vec![homework],
        checkins: vec![read20, recite, practice, handwriting],
        memos: vec![memo, memo2],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_notified_for_moves_nonempty_values_and_merges() {
        let raw = r#"{"reminders":[
            {"id":"r1","notified_for":"2026-10-17"},
            {"id":"r2","notified_for":""},
            {"id":"r3"}
        ]}"#;
        let mut runtime = RuntimeState::default();
        // 已存在的记账应被合并 / 覆盖。
        runtime.notified.insert("r1".into(), "2026-10-16".into());

        let changed = migrate_notified_for(raw, &mut runtime);
        assert!(changed);
        assert_eq!(
            runtime.notified.get("r1").map(String::as_str),
            Some("2026-10-17")
        );
        assert!(!runtime.notified.contains_key("r2"));
        assert!(!runtime.notified.contains_key("r3"));

        // 幂等：再次迁移无改动。
        assert!(!migrate_notified_for(raw, &mut runtime));
    }

    #[test]
    fn migrate_notified_for_ignores_non_json() {
        let mut runtime = RuntimeState::default();
        assert!(!migrate_notified_for("not json", &mut runtime));
        assert!(runtime.notified.is_empty());
    }

    #[test]
    fn sync_settings_roundtrip_and_legacy_defaults() {
        let mut settings = SyncSettings {
            server_url: "http://host:8080".into(),
            user_id: Some(3),
            username: "xiaoman".into(),
            token: "secret".into(),
            last_seq: 182,
            push_watermark_ms: 1_750_000_000_000,
            ..Default::default()
        };
        let text = serde_json::to_string(&settings).unwrap();
        let back: SyncSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.server_url, "http://host:8080");
        assert_eq!(back.user_id, Some(3));
        assert_eq!(back.last_seq, 182);
        assert_eq!(back.push_watermark_ms, 1_750_000_000_000);
        assert!(back.device_id.is_empty());

        // 旧版 sync.json（无 user_id / 游标字段）→ serde default。
        let legacy: SyncSettings =
            serde_json::from_str(r#"{"server_url":"http://h","token":"t"}"#).unwrap();
        assert_eq!(legacy.user_id, None);
        assert_eq!(legacy.last_seq, 0);
        assert_eq!(legacy.push_watermark_ms, 0);

        settings.device_id = "dev-x".into();
        let encoded = serde_json::to_string(&settings).unwrap();
        assert!(encoded.contains("dev-x"));
    }

    #[test]
    fn runtime_state_roundtrip() {
        let mut state = RuntimeState::default();
        state.notified.insert("r1".into(), "2026-10-17".into());
        let text = serde_json::to_string(&state).unwrap();
        let back: RuntimeState = serde_json::from_str(&text).unwrap();
        assert_eq!(
            back.notified.get("r1").map(String::as_str),
            Some("2026-10-17")
        );
        // 缺失字段 → 空 map。
        let empty: RuntimeState = serde_json::from_str("{}").unwrap();
        assert!(empty.notified.is_empty());
    }
}
