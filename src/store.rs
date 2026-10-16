use crate::model::Database;
use std::fs;
use std::path::PathBuf;

/// 数据文件位置。首次运行时写入一份示例数据，方便直接看到效果。
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

pub fn load() -> Database {
    let path = data_file();
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<Database>(&text).unwrap_or_else(|_| {
            let db = seed();
            let _ = save(&db);
            db
        }),
        Err(_) => {
            // 首次运行：写入一份示例数据，打开就能看到完整效果。
            let db = seed();
            let _ = save(&db);
            db
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

fn seed() -> Database {
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

    let session = |course_id: &str, weekday: u8, start: &str, end: &str, room: &str| Session {
        id: new_id(),
        course_id: course_id.into(),
        weekday,
        start: start.into(),
        end: end.into(),
        room: room.into(),
    };

    let sessions = vec![
        session(&chinese_id, 1, "08:00", "08:45", "A-201"),
        session(&math_id, 1, "08:55", "09:40", "A-302"),
        session(&english_id, 1, "10:00", "10:45", "B-105"),
        session(&gym_id, 1, "14:00", "14:45", "操场"),
        session(&math_id, 2, "08:00", "08:45", "A-302"),
        session(&english_id, 2, "08:55", "09:40", "B-105"),
        session(&chinese_id, 2, "10:00", "10:45", "A-201"),
        session(&math_id, 3, "08:00", "08:45", "A-302"),
        session(&art_id, 3, "09:50", "10:35", "美术楼"),
        session(&chinese_id, 3, "14:00", "14:45", "A-201"),
        session(&math_id, 4, "08:00", "08:45", "A-302"),
        session(&english_id, 4, "08:55", "09:40", "B-105"),
        session(&chinese_id, 5, "08:00", "08:45", "A-201"),
        session(&art_id, 5, "10:00", "10:45", "美术楼"),
        session(&piano_id, 6, "09:00", "10:00", "琴房 3F"),
        session(&olympic_id, 6, "10:20", "11:50", "培训中心 201"),
        session(&olympic_id, 7, "09:30", "11:00", "培训中心 201"),
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

    let homework = Reminder {
        id: new_id(),
        title: "提交数学作业".into(),
        content: "练习册 P32-33".into(),
        due_at: format!("{} 20:00", today()),
        repeat: Repeat::Daily,
        done: false,
        notified_for: String::new(),
    };

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
    }
}
