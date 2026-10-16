//! course-plan-server 二进制入口（Phase 1 / 泳道 B1）。

#[tokio::main]
async fn main() {
    if let Err(error) = course_plan_server::run().await {
        eprintln!("course-plan-server fatal: {error:#}");
        std::process::exit(1);
    }
}
