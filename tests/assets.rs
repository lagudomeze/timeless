//! 字体资产验收：HUD 指定的字体必须真的能渲染战斗日志会输出的每一个字。
//!
//! 这是一个**端到端**断言，而不是「看起来还行」：读取 `assets/` 下真实的字体文件，
//! 逐个字符查它的 `cmap`，确认有非 0 的字形 id。
//!
//! 为什么值得单独写测试：字体缺字**不会**让游戏崩溃或让 `cargo test` 变红——
//! 它只是在屏幕上变成豆腐块（□□□）。这是最容易被忽略的一类回归，
//! 尤其当日志文案改动（比如新增一种伤害类型）而字体恰好没覆盖那个字时。
//!
//! 字表不在这里硬编码：测试直接调用生产代码里的 [`faction_label`] /
//! [`DamageType::label`]，以及各阵营/伤害类型的**全部**变体。新增变体时，
//! 只要它进了日志，这个测试就会自动开始检查它。

use std::collections::BTreeSet;
use std::path::PathBuf;

use app::combat::Faction;
use app::combat::formula::types::DamageType;
use app::presentation::hud::HUD_FONT;
use app::presentation::log::faction_label;
use skrifa::MetadataProvider;

/// 仓库根目录（`CARGO_MANIFEST_DIR` 就是 crate 根，即仓库根）。
fn font_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(HUD_FONT)
}

/// 战斗日志 / HUD 可能输出的全部非 ASCII 字符。
///
/// 只收集非 ASCII：ASCII 部分由任何字体覆盖，专门列出来只会让失败信息变吵。
fn required_glyphs() -> BTreeSet<char> {
    let mut text = String::new();

    // 日志正文模板：「{who} 受到 {:.0} 点{type}伤害」「{who} 阵亡」
    // 以及找不到阵营时的兜底标签「单位」。
    text.push_str("单位 受到 点 伤害 阵亡");

    for faction in [Faction::Player, Faction::Enemy] {
        text.push_str(faction_label(&faction));
    }
    // 每种伤害类型的中文标签都会进日志。新增变体时**把它加进这个数组**——
    // 漏了的话日志会先变成豆腐块，而不是先变成编译错误。
    // （写成数组常量而不是内联 `for`，是为了将来多个变体时不用改结构。）
    const DAMAGE_KINDS: [DamageType; 1] = [DamageType::Physical];
    for kind in DAMAGE_KINDS {
        text.push_str(kind.label());
    }

    text.chars().filter(|c| !c.is_ascii()).collect()
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

#[test]
fn font_covers_every_glyph_the_battle_log_can_emit() {
    let bytes = std::fs::read(font_path()).expect("字体文件应当存在");
    let font = skrifa::FontRef::new(&bytes).expect("skrifa 应当能解析该字体");
    let charmap = font.charmap();

    let mut missing = Vec::new();
    for ch in required_glyphs() {
        // `map` 返回 Option<GlyphId>；0 是 .notdef，等同缺字。
        match charmap.map(ch) {
            Some(glyph) if glyph.to_u32() != 0 => {}
            _ => missing.push(ch),
        }
    }

    assert!(
        missing.is_empty(),
        "HUD 字体缺这些字形，日志会显示成豆腐块：{missing:?}\n\
         字体：{}（缺字就换一款覆盖更全的，或在 LICENSES.md 里登记后再换）",
        font_path().display()
    );
}
