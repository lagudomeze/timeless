//! 资产验收：HUD 字体是能被识别的 sfnt 字体，单位精灵是能当纸片用的方图。
//!
//! 这是一个**端到端**断言，而不是「看起来还行」：读 `assets/` 下真实文件的字节。
//!
//! 三条验收各守一类回归：**格式**（字体是合法 sfnt）、**字形覆盖**（界面文案里的字
//! 真的画得出来）、**贴图**（精灵是带 alpha 的方图）。它们都不会让别的测试变红，
//! 只会在屏幕上表现为豆腐块 / 人不见了 / 纸片带黑底——所以在这里钉死。
//!
//! 字形覆盖用 `skrifa` 逐字查 `cmap`；它本来就在 `bevy_text` 的依赖树里，
//! 提成直接依赖不引入新的传递依赖。

use std::path::PathBuf;

use app::combat::attack::SKILLS;
use app::presentation::hud::HUD_FONT;
use app::presentation::hud::skills::icon_path;
use app::presentation::unit_sprite::{ENEMY_SPRITE, PLAYER_SPRITE, SHADOW_SPRITE};

/// 仓库根目录（`CARGO_MANIFEST_DIR` 就是 crate 根，即仓库根）。
fn asset_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(relative)
}

/// HUD 字体在本机 `assets/` 下的绝对路径。
fn font_path() -> PathBuf {
    asset_path(HUD_FONT)
}

#[test]
fn font_asset_exists_and_is_a_font() {
    let path = font_path();
    let bytes = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "读不到 HUD 字体 {}：{error}\n（HUD_FONT = {HUD_FONT:?}，相对 assets/ 解析）",
            path.display()
        )
    });
    assert!(
        bytes.len() > 1024,
        "字体文件太小（{} 字节），大概不是真字体",
        bytes.len()
    );
    // sfnt 魔数：0x00010000（TrueType）或 "OTTO"（CFF/OpenType）
    let magic = &bytes[..4];
    assert!(
        magic == [0x00, 0x01, 0x00, 0x00] || magic == *b"OTTO" || magic == *b"true",
        "不是可识别的 sfnt 字体（魔数 {magic:02X?}）"
    );
}

/// 读 PNG 的 IHDR 头：返回（宽, 高, 颜色类型）。
///
/// 只认 PNG 规范里定长的那几个字段，不需要解码像素——验收要的是「文件是不是真的
/// 是带 alpha 的方图」，而不是图片内容。
fn png_header(bytes: &[u8]) -> (u32, u32, u8) {
    assert!(bytes.len() > 33, "PNG 文件不该这么小");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "不是 PNG（魔数不对）");
    assert_eq!(&bytes[12..16], b"IHDR", "PNG 的第一个数据块应当是 IHDR");
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (width, height, bytes[25])
}

/// 单位精灵验收：玩家 / 敌人 / 阴影三张贴图必须存在、是**方图**、且带 alpha 通道。
///
/// 为什么值得单独写测试：缺文件或丢了 alpha 都不会让 `cargo test` 变红，屏幕上的
/// 表现却分别是「人不见了」和「纸片带一块黑底」——都是最容易被忽略的一类回归。
/// 方图则是为了让贴图按 1:1 铺在纸片上（见 `presentation::unit_sprite::SPRITE_SIZE`）。
#[test]
fn unit_sprites_exist_square_and_keep_transparency() {
    const COLOR_TYPE_RGBA: u8 = 6;
    // 技能图标也一起验收：注册表里每加一个技能，它的图标就会被要求存在且合法
    let icons: Vec<&str> = SKILLS.iter().map(|def| icon_path(def.kind)).collect();
    let paths = [PLAYER_SPRITE, ENEMY_SPRITE, SHADOW_SPRITE]
        .into_iter()
        .chain(icons);
    for path in paths {
        let file = asset_path(path);
        let bytes = std::fs::read(&file)
            .unwrap_or_else(|error| panic!("读不到单位精灵 {}：{error}", file.display()));
        let (width, height, color_type) = png_header(&bytes);
        assert_eq!(
            color_type, COLOR_TYPE_RGBA,
            "{path} 必须带 alpha 通道：精灵是透明底纸片，缺了会糊上一块底色"
        );
        assert_eq!(
            width, height,
            "{path} 应当是方图（{width}x{height}）：纸片按 1:1 贴，非方图会被拉变形"
        );
        assert!(width >= 16, "{path} 只有 {width}px，做成纸片会糊");
    }
}

/// **字形覆盖验收**：HUD 字体必须真的能画出界面上会出现的字。
///
/// 已有的那条只验"文件在、是合法 sfnt"——**合法但缺字**的字体照样通过，
/// 而症状是屏幕上出现豆腐块（□□），只在运行时肉眼可见。这条把那次人工确认
/// 变成自动验收：逐字查 `cmap`。
///
/// `skrifa` 本来就在 `bevy_text` 的依赖树里（这里只是提成直接依赖），
/// 所以不必为了这条测试引入新的传递依赖。
#[test]
fn the_font_covers_every_character_the_ui_can_show() {
    use skrifa::MetadataProvider;

    let path = font_path();
    let bytes = std::fs::read(&path).expect("字体文件应当在");
    let font = skrifa::FontRef::new(&bytes).expect("字体应当能解析成 FontRef");
    let charmap = font.charmap();

    // 界面上真正会出现的字：HUD 英文 + 战斗日志中文 + 常用标点与数字。
    // 覆盖不全的症状就是豆腐块，所以这里逐个查而不是抽查。
    let ui_text = [
        // HUD（英文）
        "PLAYER ENEMY HP EN TIMELINE FROZEN RUNNING RUNNING SKILL COST WINDUP RECOVERY POWER",
        "attack melee fireball roll parry move jump shoot wait",
        "0123456789:/.-+%()[]·",
        // 战斗日志正文（中文）
        "敌人玩家火球近战横扫箭矢翻滚招架格挡命中伤害打断撤销掉落死亡重生",
        "前摇后摇决策槽时间线冻结威胁反应反制精力护甲暴击闪避无敌",
        "距离格子在左上下右前后目标法术技能等待暂停继续重置战斗日志",
        "的了一是在有和就不人都一个我他这",
        // 全角标点与常用符号
        "，。：、；！？（）【】「」…—·",
    ];

    let mut missing: Vec<char> = Vec::new();
    for text in ui_text {
        for ch in text.chars() {
            if charmap.map(ch).is_none() && !missing.contains(&ch) {
                missing.push(ch);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "字体 {} 缺这些字形：{:?}\n（它们在界面文案里出现过，缺了会显示成豆腐块）",
        path.display(),
        missing
    );
}
