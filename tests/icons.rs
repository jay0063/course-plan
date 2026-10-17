//! 兜底测试：源码中引用的每个 `IconName` 变体都必须登记在 `src/assets.rs` 的
//! `APP_ICONS` 中，并且确实能从应用资产源加载到对应 SVG。
//!
//! 若有人新增了一个图标引用却忘记登记，本测试会失败。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use course_plan::assets::{APP_ICONS, AppAssets};
use course_plan::gpui::AssetSource;
use course_plan::gpui::assets::IconName;

const ICON_PREFIX: &str = "IconName::";

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("读取 {} 失败: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("目录项读取失败").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// 提取源码中每一个 `IconName::<Name>` 的 `<Name>` 部分。
fn collect_icon_tokens(source: &str, out: &mut BTreeSet<String>) {
    let mut cursor = source;
    while let Some(idx) = cursor.find(ICON_PREFIX) {
        cursor = &cursor[idx + ICON_PREFIX.len()..];
        let name: String = cursor
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let len = name.len();
        if len > 0 {
            out.insert(name);
        }
        cursor = &cursor[len..];
    }
}

/// `CalendarDays` -> `calendar-days`，与 IconName 的资产路径命名一致。
/// 数字同样算作独立词段：`Link2` -> `link-2`、`Link2Off` -> `link-2-off`。
fn pascal_to_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let mut prev: Option<char> = None;
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i != 0 {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_ascii_digit() {
            if prev.is_some_and(|p| p.is_ascii_alphabetic()) {
                out.push('-');
            }
            out.push(ch);
        } else {
            out.push(ch);
        }
        prev = Some(ch);
    }
    out
}

#[test]
fn every_used_icon_is_registered_and_loadable() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(!files.is_empty(), "未在 {} 下找到源码文件", src.display());

    let mut tokens = BTreeSet::new();
    for file in &files {
        let source = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("读取 {} 失败: {e}", file.display()));
        collect_icon_tokens(&source, &mut tokens);
    }

    // 自检：确保 token 提取有效（否则测试会因「无图标」而空转通过）。
    assert!(tokens.contains("Bell"), "图标 token 提取失败: {tokens:?}");
    assert!(tokens.len() >= 20, "图标 token 数量异常: {tokens:?}");

    let registered: BTreeSet<String> = APP_ICONS
        .iter()
        .map(|icon| icon.path().to_string())
        .collect();
    let catalog: BTreeSet<String> = IconName::ALL
        .iter()
        .map(|icon| icon.path().to_string())
        .collect();

    for token in &tokens {
        let path = format!("icons/{}.svg", pascal_to_snake(token));
        assert!(
            catalog.contains(&path),
            "`IconName::{token}` 解析为 `{path}`，但这不是一个已知图标"
        );
        assert!(
            registered.contains(&path),
            "`IconName::{token}`（`{path}`）被引用，但未登记到 src/assets.rs 的 APP_ICONS"
        );

        let bytes = AppAssets
            .load(&path)
            .unwrap_or_else(|e| panic!("资产源加载 `{path}` 出错: {e}"))
            .unwrap_or_else(|| panic!("资产源对 `{path}` 返回空，图标将无法渲染"));
        let svg =
            std::str::from_utf8(&bytes).unwrap_or_else(|e| panic!("`{path}` 不是合法 UTF-8: {e}"));
        assert!(svg.contains("<svg"), "`{path}` 内容不是 SVG");
    }
}
