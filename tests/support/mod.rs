//! 无头渲染 / 截图测试的共享脚手架（Phase 1 · 泳道 V）。
//!
//! 这里把「真实渲染 + 截图」串起来，作为后续设计阶段（Phase 3/4）的视觉证据管线：
//!
//! - [`HeadlessAppContext`]（`TestDispatcher` + 可选 `WgpuHeadlessRenderer`）渲染真实窗口；
//! - 用 [`TestWindowExt`] 派发事件（find / click / input / render_frame）；
//! - 用 `Window::render_to_image` 把当前帧抓成 RGBA，再存成 PNG。
//!
//! 默认不跑：只有环境变量 `COURSEPLAN_VISUAL=1` 时才启用（见 [`enabled`]），
//! 这样在没有 GPU / lavapipe 的机器上普通 `cargo test` 仍保持绿色。
//!
//! 依赖（根 `Cargo.toml` `[dev-dependencies]`）：
//! - `gpui-kit = { version = "0.7", features = ["test-support"] }`：暴露 `gpui::test` 与
//!   `HeadlessAppContext` / `render_to_image` / `WgpuHeadlessRenderer`；
//! - `image`：保存 PNG。

#![allow(dead_code)] // 不同测试文件按需使用其中一部分辅助函数

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use course_plan::app::{APP_ID, App, Page};
use course_plan::gpui::component::{Theme, ThemeMode};
use course_plan::gpui::test::TestWindowExt;
use course_plan::gpui::{
    AnyWindowHandle, AppContext, AssetSource, Entity, HeadlessAppContext, px, size,
};
use course_plan::model::Database;
use course_plan::store;
use course_plan::views::courses::CourseDraft;
use image::RgbaImage;

/// 环境变量：置为 `1` 才真正渲染 + 截图。
pub const VISUAL_ENV: &str = "COURSEPLAN_VISUAL";

/// 本机 lavapipe（软件 Vulkan）ICD 路径，供无 GPU 环境的错误提示使用。
pub const LAVAPIPE_ICD: &str = "/usr/share/vulkan/icd.d/lvp_icd.json";

/// 渲染窗口的逻辑尺寸（与 `lib.rs::run()` 中的窗口一致）。
pub const WINDOW_W: f32 = 1280.;
pub const WINDOW_H: f32 = 840.;

/// 是否启用真实渲染（`COURSEPLAN_VISUAL=1`）。
pub fn enabled() -> bool {
    std::env::var(VISUAL_ENV).as_deref() == Ok("1")
}

/// 打印跳过信息并直接返回（各测试在未启用时调用后 `return`）。
pub fn skip(test: &str) {
    eprintln!(
        "[render] 跳过 {test}：未启用无头渲染。设置 {VISUAL_ENV}=1 以运行：\n\
         \t{VISUAL_ENV}=1 cargo test --test render -- --nocapture"
    );
}

/// 截图输出目录：`target/visual/`（位于已 gitignore 的 `target/` 内）。
pub fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("visual")
}

/// 资源来源：与 `lib.rs::run()` 一致，使用 `src/assets.rs::AppAssets`
/// （gpui-kit 默认图标集 + 应用额外 8 个图标的组合），确保截图里图标能正常渲染。
fn asset_source() -> Arc<dyn AssetSource> {
    Arc::new(course_plan::assets::AppAssets)
}

/// 把渲染器初始化失败转换成带精确 env 提示的信息。
pub fn renderer_hint(err: &dyn std::fmt::Display) -> String {
    format!(
        "无法初始化无头 GPU 渲染器：{err}\n\
         \n\
         本机需要可用的 Vulkan/GL 适配器。无独显时请启用 lavapipe（软件 Vulkan）：\n\
         \tVK_ICD_FILENAMES={LAVAPIPE_ICD} {VISUAL_ENV}=1 cargo test --test render -- --nocapture\n\
         （若缺少 {LAVAPIPE_ICD}，请安装 mesa-vulkan-drivers / mesa 的 lavapipe 软件驱动。）"
    )
}

/// 无头应用脚手架：持有 headless context、窗口句柄与根 `App` 实体。
pub struct Harness {
    pub cx: HeadlessAppContext,
    pub window: AnyWindowHandle,
    pub app: Entity<App>,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    /// 用 `store::sample_data()` 构造脚手架（确定的示例数据，不读写磁盘）。
    pub fn new() -> Self {
        Self::with_database(store::sample_data())
    }

    /// 用注入的 [`Database`] 构造脚手架。
    pub fn with_database(db: Database) -> Self {
        // 真实字形度量：用 headless 平台的文本系统（CosmicTextSystem）。
        let text_system = course_plan::gpui::platform::current_platform(true).text_system();

        let mut cx = HeadlessAppContext::with_platform(
            text_system,
            asset_source(),
            // 每次开窗时创建真实无头渲染器（Linux: WgpuHeadlessRenderer, VULKAN|GL）。
            course_plan::gpui::platform::current_headless_renderer,
        );

        // 组件层初始化（主题、按键绑定、全局状态）——对应 `lib.rs::run()` 的 `init(cx)`。
        cx.update(course_plan::gpui::init);
        // 组件库内置文案切中文——与 `lib.rs::run()` 一致（日历月份/星期等）。
        course_plan::gpui::component::set_locale("zh-CN");
        // 截图管线在 reduce-motion 下取帧：页面转场（opacity 0→1）等入场动画
        // 会直接渲染终态，PNG 才是"稳定态"而不是动画第一帧的空白。
        cx.update(|app| app.set_reduce_motion(true));
        // 装载 CoursePlan 主题（与 `lib.rs::run()` 一致；幂等）。
        cx.update(course_plan::theme::init);
        cx.update(|app| app.set_app_identity(APP_ID, "CoursePlan"));

        let handle = cx
            .open_window(size(px(WINDOW_W), px(WINDOW_H)), move |_window, cx| {
                cx.new(|_| App::with_database(db))
            })
            .unwrap_or_else(|err| panic!("{}", renderer_hint(&err)));

        let window: AnyWindowHandle = handle.into();
        let app = cx
            .read_window(&handle, |entity, _app| entity)
            .expect("读取根 App 实体失败");

        // 默认浅色，与 run() 一致。
        let mut harness = Self { cx, window, app };
        harness.set_theme(ThemeMode::Light);
        harness
    }

    /// 切换页面（走 `App::goto`）。
    pub fn set_page(&mut self, page: Page) {
        let app = self.app.clone();
        app.update(&mut self.cx, |app, cx| app.goto(page, cx));
    }

    /// 切换主题。
    ///
    /// 走 `theme::apply_for_mode` 而非直接 `Theme::change`——后者命中 registry
    /// 默认配置（gpui-component 内置），不会自动套用我们的 CoursePlan 调色板。
    pub fn set_theme(&mut self, mode: ThemeMode) {
        self.cx
            .update_window(self.window, |_, window, cx| {
                Theme::change(mode, Some(window), cx);
                course_plan::theme::apply_for_mode(cx);
            })
            .expect("切换主题失败");
        self.draw();
    }

    /// 刷新并绘制一帧（`TestWindowExt::render_frame`）。
    pub fn draw(&mut self) {
        self.cx
            .update_window(self.window, |_, window, cx| {
                window.render_frame(cx);
            })
            .expect("绘制一帧失败");
    }

    /// 抓取当前帧为 RGBA 图。
    pub fn screenshot(&mut self) -> RgbaImage {
        self.cx
            .capture_screenshot(self.window)
            .unwrap_or_else(|err| panic!("{}", renderer_hint(&err)))
    }

    /// 期望的截图尺寸（设备像素）= 逻辑尺寸 × 缩放因子。
    ///
    /// `TestWindow` 的缩放因子固定为 2.0，因此 1280×840 的窗口会产出 2560×1680 的图。
    pub fn expected_device_size(&mut self) -> (u32, u32) {
        self.cx
            .update_window(self.window, |_, window, _| {
                let device =
                    size(px(WINDOW_W), px(WINDOW_H)).to_device_pixels(window.scale_factor());
                (device.width.0 as u32, device.height.0 as u32)
            })
            .expect("读取缩放因子失败")
    }

    /// 渲染指定页面 + 主题，存成 `target/visual/<name>.png`，返回 (路径, 图)。
    pub fn capture_page(
        &mut self,
        page: Page,
        mode: ThemeMode,
        name: &str,
    ) -> (PathBuf, RgbaImage) {
        self.set_page(page);
        self.set_theme(mode);
        self.draw();
        self.capture_current(name)
    }

    /// 抓取当前帧并保存为 `target/visual/<name>.png`。
    pub fn capture_current(&mut self, name: &str) -> (PathBuf, RgbaImage) {
        let image = self.screenshot();
        let path = output_dir().join(format!("{name}.png"));
        save_png(&image, &path);
        (path, image)
    }

    // MARK: - 交互辅助

    /// 通过公开 API 打开「新建课程」编辑器（`App::open_course_editor`）。
    pub fn open_course_editor(&mut self) {
        let app = self.app.clone();
        self.cx
            .update_window(self.window, |_, window, cx| {
                app.update(cx, |app, cx| {
                    app.open_course_editor(CourseDraft::empty(), window, cx);
                });
            })
            .expect("打开课程编辑器失败");
    }

    /// 关闭所有编辑器遮罩（`ui::widgets::close_editor`）。
    pub fn close_editors(&mut self) {
        let app = self.app.clone();
        self.cx
            .update_window(self.window, |_, window, cx| {
                course_plan::ui::widgets::close_editor(app, cx);
                window.render_frame(cx);
            })
            .expect("关闭编辑器失败");
    }

    /// 向当前焦点派发文本输入（`TestWindowExt::input`）。
    ///
    /// 说明：GPUI 的键盘 `input` 发往「当前焦点」。当前视图尚未注册
    /// `.test_support().track_focus(...)`（属 Phase 3/4 的改造范围），因此这一步在真实
    /// 焦点绑定落地前是 best-effort：不 panic，但文本未必进入控件。
    pub fn input(&mut self, text: &str) {
        self.cx
            .update_window(self.window, |_, window, cx| {
                window.input(text, cx);
            })
            .expect("文本输入失败");
    }

    /// 按键（`TestWindowExt::press`）。
    pub fn press(&mut self, key: &str) {
        self.cx
            .update_window(self.window, |_, window, cx| {
                window.press(key, cx);
            })
            .expect("按键失败");
    }
}

/// 保存 RGBA 图为 PNG（自动创建父目录）。
pub fn save_png(image: &RgbaImage, path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("创建 {} 失败：{err}", parent.display()));
    }
    image
        .save(path)
        .unwrap_or_else(|err| panic!("保存 {} 失败：{err}", path.display()));
}

/// 断言截图非平凡：尺寸符合预期，且采样像素不止一种颜色（不是纯色/空白）。
pub fn assert_nontrivial(image: &RgbaImage, expect_w: u32, expect_h: u32, name: &str) {
    assert_eq!(image.width(), expect_w, "{name}: 截图宽度不符");
    assert_eq!(image.height(), expect_h, "{name}: 截图高度不符");

    let (w, h) = (image.width(), image.height());
    let samples = [
        (0, 0),
        (w / 4, h / 4),
        (w / 2, h / 2),
        (3 * w / 4, h / 4),
        (w / 4, 3 * h / 4),
        (3 * w / 4, 3 * h / 4),
        (w - 1, 0),
        (0, h - 1),
        (w - 1, h - 1),
    ];
    let colors: HashSet<[u8; 4]> = samples
        .iter()
        .map(|&(x, y)| image.get_pixel(x, y).0)
        .collect();
    assert!(
        colors.len() > 1,
        "{name}: 截图近乎纯色（只在 {} 个采样点看到 1 种颜色），疑似空白/渲染失败",
        samples.len()
    );
}

/// 两张图是否存在像素差异（用于交互冒烟：帧确实变了）。
pub fn images_differ(a: &RgbaImage, b: &RgbaImage) -> bool {
    if a.width() != b.width() || a.height() != b.height() {
        return true;
    }
    let step = 4; // 稀疏采样，够用且快
    let (w, h) = (a.width(), a.height());
    for y in (0..h).step_by(step as usize) {
        for x in (0..w).step_by(step as usize) {
            if a.get_pixel(x, y) != b.get_pixel(x, y) {
                return true;
            }
        }
    }
    false
}
