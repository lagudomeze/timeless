//! # hud — 战斗 HUD（拆成面板 / 技能栏 / 时间轴 / 日志 / 帮助）
//!
//! **引擎版本：bevy 0.19.1**（`Cargo.toml` 写 `bevy = "0.19"`，`Cargo.lock` 锁 0.19.1）。
//! 0.16 起 UI 的 `NodeBundle` / `TextBundle` / `UiImage` 已删除：现在 UI 就是**普通组件**
//! （`Node` + `BackgroundColor` + `ImageNode` + `Text`…），`Style` 的字段并进了 `Node`，
//! `BorderRadius` 也不再是组件而是 `Node` 的字段。
//!
//! 旧的「左上角一坨纯文本」被拆成五块，各自有独立的文件、组件与更新系统：
//!
//! | 屏幕位置 | 内容 | 文件 |
//! | :--- | :--- | :--- |
//! | 顶部 | 时间轴：按执行时刻排的行动色块（蓝 = 玩家、红 = 敌人） | [`timeline`] |
//! | 左下 | 玩家：头像 + HP / EN 条 + 状态行 | [`panels`] |
//! | 右下 | 敌人：同上（镜像） | [`panels`] |
//! | 底部居中 | 技能栏：图标按钮 + 消耗角标 + 悬停 tooltip | [`skills`] |
//! | 右下偏上 | 战斗日志：半透明、点标题折叠 | [`log_panel`] |
//! | 居中 | 帮助面板：`F1` 开合（常驻按键提示已移除） | [`help`] |
//!
//! 三条不变量：
//!
//! 1. **只读**：HUD 只把游戏状态映射成 `Node` / `Text`，绝不写回游戏数据；
//! 2. **文案英文**：中文只出现在战斗日志正文（见 [`crate::presentation::log`]）；
//! 3. **单一字体**：每段文本都显式用 [`HUD_FONT`]（Bevy 默认字体不含 CJK，
//!    日志正文会变豆腐块）。
//!
//! 「滑动条」在 HUD 里是**只读进度条**：表现层不改 `Health` / `Stamina`，
//! 条长就是把数值比例映射成 `Val::Percent`（见 [`panels::bar_fraction`]）。
//!
//! **实体命名**：每个 HUD 节点都挂 `Name`，格式是「区域 + 职责」，例如
//! `PlayerPanel` / `PlayerHpFill` / `SkillSlot2Badge` / `TimelineBlock3`。
//! 这些名字不是装饰：BRP 的 `world.query`、`world_find_entities_by_name`、
//! 按名截图都靠它们定位实体（UI 树里父子关系深，没有名字就只能靠 ID 猜）。
//! HUD 的标记组件同时用 `app.register_type` 注册进反射表（见
//! [`crate::presentation::PresentationPlugin`]）——**没注册的组件在 BRP 里等于不存在**，
//! 加了新标记组件记得一起补注册。

use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;

pub mod actions;
pub mod help;
pub mod hint;
pub mod layout;
pub mod log_panel;
pub mod panels;
pub mod skills;
pub mod timeline;

pub use actions::update_action_labels_system;
pub use help::{ToggleHelp, toggle_help_system};
pub use hint::{ActionHint, ActionHintText, HintTimer, PreviewReadout, update_action_hint_system};
pub use layout::{HudRoot, fit_ui_scale_system, setup_hud};
pub use log_panel::{toggle_log_system, update_log_panel_system};
pub use panels::update_unit_panels_system;
pub use skills::{SkillTooltip, update_skill_bar_system};
pub use timeline::{
    TimelineReadout, TimelineReadoutText, TimelineStateLabel, update_timeline_readout_system,
    update_timeline_system,
};

/// HUD 写入缓存：**只有内容真的变了才碰 UI 节点**。
///
/// 为什么不直接用 `Changed<Health>` 这类过滤器：UI 文本是**派生值**，会漏掉
/// 「单位被销毁」（没有组件变化可查）、「两个不同字段换算出同一句话」这些情况，
/// 而状态行同时依赖血量 / 精力 / 格子 / 姿态。所以这里按**快照比对**：
/// 每帧只算一份便宜的纯数据快照，与上一帧相等就整帧不碰 UI。
///
/// 等输入冻结时收益尤其明显：玩家等输入时虚拟时间冻结、单位不动，HUD 长期处于
/// 「无变化」状态，省掉的就是每帧白写的那些 `Text` / `Node` 变更。
/// 唯一的例外是时间轴——虚拟时间在走时色块每帧都在移动，那时它本来就该重画。
#[derive(Resource, Default)]
pub struct HudCache {
    pub units: panels::UnitPanelCache,
    pub actions: actions::ActionLabelCache,
    pub timeline: timeline::TimelineCache,
    pub skills: skills::SkillBarCache,
    pub log: log_panel::LogCache,
}

/// HUD 用的字体资产路径。
///
/// 选 Noto Sans SC：OFL-1.1、简体覆盖全，且能同时渲染英文与 CJK
/// （因此不需要字体回退链）。资产与许可见 `assets/LICENSES.md`。
pub const HUD_FONT: &str = "fonts/NotoSansSC-Regular.otf";

/// UI 缩放的设计基准高度（像素）：`fit_ui_scale_system` 按窗口高度 / 它来缩放。
pub const BASE_HEIGHT: f32 = 720.0;
/// UI 缩放的上下限（防止超宽 / 超小窗口把 HUD 拉爆）。
pub const MIN_UI_SCALE: f32 = 0.75;
/// 见 [`MIN_UI_SCALE`]。
pub const MAX_UI_SCALE: f32 = 1.6;

/// HUD 面板底色（半透明深色）。
pub const PANEL_BG: Color = Color::srgba(0.05, 0.06, 0.09, 0.72);
/// HUD 条 / 槽位的轨道底色。
pub const TRACK_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.45);
/// HP 条颜色。
pub const HP_COLOR: Color = Color::srgb(0.85, 0.29, 0.32);
/// EN（精力）条颜色。
pub const EN_COLOR: Color = Color::srgb(0.35, 0.62, 0.95);

/// 阵营主色（时间轴色块、面板描边共用）。
pub fn faction_color(faction: crate::combat::Faction) -> Color {
    faction_color_alpha(faction, 1.0)
}

/// 带透明度的阵营色（未提交的草案用半透明表示）。
pub fn faction_color_alpha(faction: crate::combat::Faction, alpha: f32) -> Color {
    match faction {
        crate::combat::Faction::Player => Color::srgba(0.30, 0.55, 0.95, alpha),
        crate::combat::Faction::Enemy => Color::srgba(0.88, 0.38, 0.38, alpha),
    }
}

/// 载入一张 UI 贴图（像素风：最近邻采样，避免被线性过滤糊掉）。
pub fn load_ui_image(assets: &AssetServer, path: &'static str) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::nearest();
        })
        .load::<Image>(path)
}

/// 一行 HUD 文本。
///
/// 包成**一个 Bundle** 而不是裸元组：`children![]` 的元素里再嵌一个元组时，
/// 宏的展开会把它当成一个整体，写成一个具名 Bundle 最省事。
#[derive(Bundle)]
pub struct HudText {
    text: Text,
    font: TextFont,
    color: TextColor,
}

/// 造一行 HUD 文本。
pub fn hud_text(font: &Handle<Font>, size: f32, text: impl Into<String>) -> HudText {
    hud_text_tinted(font, size, text, Color::srgb(0.86, 0.89, 0.94))
}

/// 造一行带颜色的 HUD 文本（颜色必须走这里，不能再并排塞 `TextColor`：会重复）。
pub fn hud_text_tinted(
    font: &Handle<Font>,
    size: f32,
    text: impl Into<String>,
    color: Color,
) -> HudText {
    HudText {
        text: Text::new(text),
        font: TextFont::from_font_size(size).with_font(font.clone()),
        color: TextColor(color),
    }
}
