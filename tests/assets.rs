//! 资产验收：HUD 字体是能被识别的 sfnt 字体，单位精灵是能当纸片用的方图。
//!
//! 这是一个**端到端**断言，而不是「看起来还行」：读 `assets/` 下真实文件的字节。
//!
//! 刻意只查「文件在不在、格式对不对」，不查字形覆盖：逐字查 `cmap` 要引第三方
//! 字体解析库（`skrifa`），而字体本身已经随仓库发版（`assets/LICENSES.md` 有许可）。
//! 字形覆盖目前靠**运行时人工确认**（战斗日志出现中文时看有没有豆腐块），
//! release 前再决定要不要把 cmap 验收加回来（见 TODO.md 的「代码 A 专属待办」）。

use std::path::PathBuf;

use app::combat::skills::SKILLS;
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
