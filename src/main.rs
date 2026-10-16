//! 二进制入口：实际逻辑在 `course_plan` 库中（便于集成测试与无头渲染测试）。

fn main() {
    course_plan::run();
}
