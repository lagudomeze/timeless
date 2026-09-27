"""重打 HUD 字体的子集：字符集与 tests/assets.rs 的真相源**逐字对齐**。

用法（仓库根目录）：
    python tools/subset_font.py <源字体.otf> assets/fonts/NotoSansSC-Regular.otf

字符集的来源与 tests/assets.rs::the_font_covers_every_character_the_ui_can_show 一致：
  1. 那条测试里写死的 ui_text（ASCII + 全角标点 + 拉丁词）；
  2. `chinese_in_source()`：从四个中文正文产处抽**字符串字面量**里的汉字，
     并**跳过 `#[cfg(test)]` 模块**（断言消息里的中文永远不会渲染）。
第三条是规则，不是字表——所以这个脚本要跟住那份列表，别自己发明字。
"""

import re
import sys
from pathlib import Path

# 与 tests/assets.rs 的 `chinese_in_source()` 一览一致。
# ⚠️ 漏掉一个中文产处 = 那批字永远不会被要求进字体（2026-09-27 就漏过 `hud/hint.rs`，
# 结果威胁读数在实机上是豆腐块，而验收测试一直绿着）。两处必须同步。
SOURCES = [
    "src/presentation/log.rs",
    "src/presentation/hud/hint.rs",
    "src/presentation/hud/panels/model.rs",
    "src/presentation/hud/timeline/model.rs",
    "src/world/storage/interaction.rs",
]


def is_cjk(ch: str) -> bool:
    return "\u4e00" <= ch <= "\u9fff"


def ui_text_from_test(root: Path) -> str:
    """**从 `tests/assets.rs` 里解析** `ui_text` 那个数组，而不是手抄一份。

    手抄过一次，漏了 `n` / `B` / `g` 等 25 个字形——测试照样绿（它只查
    `chinese_in_source` 抽到的汉字，ASCII 那一段没人对账），
    而屏幕上 `awaiting` / `COMBAT` 直接变成豆腐块。
    所以这份字表只有一个真相源：那条验收测试自己。
    """
    source = (root / "tests/assets.rs").read_text(encoding="utf-8")
    start = source.index("let ui_text = [")
    end = source.index("];", start)
    block = source[start:end]
    return "".join(re.findall(r'"([^"]*)"', block))


def chinese_in_source(root: Path) -> set[str]:
    """从源码里抓会显示给玩家的汉字（跳过测试模块与注释行）。"""
    chars: set[str] = set()
    for relative in SOURCES:
        source = (root / relative).read_text(encoding="utf-8")
        depth = 0
        skipping_test: int | None = None
        previous_was_cfg_test = False
        for line in source.splitlines():
            code = line.lstrip()
            is_comment = code.startswith("//") or code.startswith("*")
            opens_test = code.startswith("#[cfg(test)]")

            if not is_comment and skipping_test is None and not previous_was_cfg_test and not opens_test:
                # 逐对取引号之间的部分（与 Rust 版 split('"').skip(1).step_by(2) 同构）
                parts = line.split('"')
                for literal in parts[1::2]:
                    chars.update(ch for ch in literal if is_cjk(ch))

            for ch in line:
                if ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if skipping_test == depth + 1:
                        skipping_test = None
            if opens_test and skipping_test is None:
                skipping_test = depth + 1
            previous_was_cfg_test = opens_test
    return chars


def build_charset(root: Path) -> str:
    chars: set[str] = set(ui_text_from_test(root))
    chars |= chinese_in_source(root)
    # 保险：把所有可打印 ASCII 都带上（HUD 是英文，任何字母都可能出现在某条读数里；
    # 而 `ui_text` 那份数组只列了"当前用到"的词）。代价是字形多一点，
    # 换来的是"改了某个英文单词就冒出豆腐块"这类问题不会再发生。
    chars |= {chr(code) for code in range(0x20, 0x7F)}
    return "".join(sorted(chars))


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    source, output = Path(sys.argv[1]), Path(sys.argv[2])
    root = Path(__file__).resolve().parent.parent
    charset = build_charset(root)

    from fontTools import subset
    from fontTools.ttLib import TTFont

    options = subset.Options()
    options.hinting = False
    options.desubroutinize = True
    options.layout_features = []
    options.drop_tables += ["DSIG"]
    options.notdef_outline = True
    options.recalc_bounds = True

    font = subset.load_font(str(source), options)
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(text=charset)
    subsetter.subset(font)
    subset.save_font(font, str(output), options)

    # 自查：重打完的字体必须覆盖同一份字符集（否则测试会红，但先在这里说清楚）
    written = TTFont(str(output))
    cmap = written.getBestCmap()
    missing = [ch for ch in charset if ord(ch) not in cmap]
    wrote = output.stat().st_size
    print(f"字符集 {len(charset)} 个字；写出 {wrote} 字节（{wrote / 1024:.1f} KB）")
    print("汉字:", "".join(sorted(ch for ch in charset if is_cjk(ch))))
    if missing:
        print("*** 缺字:", "".join(missing))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
