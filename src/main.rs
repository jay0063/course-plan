//! 二进制入口：实际逻辑在 `course_plan` 库中（便于集成测试与无头渲染测试）。
//!
//! Windows 上 Release 构建使用 GUI 子系统：不加这条时 Rust 默认生成 console
//! 子系统程序，双击 exe / 开始菜单快捷方式都会先弹一个黑色终端窗口。
//! 仅对 release 生效，debug 构建仍保留控制台以便看日志。
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    course_plan::run();
}
