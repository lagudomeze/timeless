# 素材许可清单（assets/）

> 逐项记录来源与许可证；新增素材必须在此登记。

| 路径 | 来源 | 许可证 | 备注 |
| :--- | :--- | :--- | :--- |
| `textures/ground/grass.png` | Kenney.nl Prototype Textures（`PNG/Green/texture_01.png`） | CC0 1.0 | https://kenney.nl/assets/prototype-textures · ⚠️ **不要接进渲染**：这是**灰盒原型贴图**，纯绿底 + 白网格 + 烙在图上的文字「WALL / 1 × 1 meter / 1024 × 1024」，铺开会把说明文字重复到整个地形上。见 TODO.md 的对应条目 |
| `textures/units/player.png` | Kenney.nl Tiny Dungeon（`Tiles/tile_0097.png`，骑士） | CC0 1.0 | https://kenney.nl/assets/tiny-dungeon · 原件是 16×16 调色板 PNG，**无损转成 RGBA**（见下） |
| `textures/units/enemy.png` | Kenney.nl Tiny Dungeon（`Tiles/tile_0121.png`，幽灵） | CC0 1.0 | 同上 |
| `textures/units/shadow.png` | 本仓库程序生成（软边黑圆，128×128 RGBA） | 无（自有素材） | 贴地阴影：64 层同心椭圆叠加 alpha 得到径向渐变 |
| `textures/ui/icon_*.png`（4 个：attack / melee / fireball / roll） | 本仓库程序生成（64×64 RGBA 技能图标：靶心 / 剑 / 火球 / 弧形箭头） | 无（自有素材） | 技能栏占位图标；换正式图标时替换同名文件即可 |
| `textures/ui/icon_shoot.png` | Kenney.nl Tiny Dungeon（`Tiles/tile_0119.png`，弓） | CC0 1.0 | https://kenney.nl/assets/tiny-dungeon · 与单位精灵同一套素材（风格一致）· 原件 16×16，**最近邻放大到 64×64**（HUD 用 `ImageSampler::nearest`，放大后仍是硬边像素）· 单体射击（箭矢）技能的图标 |
| `models/nature/*.glb`（21 个） | Kenney.nl Nature Kit（`Models/GLTF format/*.glb`） | CC0 1.0 | https://kenney.nl/assets/nature-kit |
| `fonts/NotoSansSC-Regular.otf` | noto-cjk 仓库 `Sans/SubsetOTF/SC/NotoSansSC-Regular.otf` | OFL-1.1 | https://github.com/googlefonts/noto-cjk · **已子集化：8.3 MB → 16 KB**（154 个字形，配方见下节与 `tools/subset_font.py`）· 单一 Regular 字重 |

下载直链（zip 内 License.txt 亦随包提供）：

- https://kenney.nl/media/pages/assets/prototype-textures/a88c69fa18-1677578307/kenney_prototype-textures.zip
- https://kenney.nl/media/pages/assets/tiny-dungeon/f8422efb44-1674742415/kenney_tiny-dungeon.zip
- https://kenney.nl/media/pages/assets/nature-kit/37ac38a37b-1677698939/kenney_nature-kit.zip
- https://cdn.jsdelivr.net/gh/googlefonts/noto-cjk@main/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf

CC0 1.0 无需署名，可自由商用与修改；OFL-1.1 允许自由使用、修改与再分发（修改后沿用 OFL，名称需避让保留字体名）。

## 单位精灵为什么转成 RGBA

Kenney 的单张图块是**调色板 PNG**（color type 3 + `tRNS`）：透明是有的，但不带真正的
alpha 通道。这里统一转成 32 位 RGBA（color type 6）：

- 像素一一对应，转换无损，颜色与形状都没有改动；
- 验收测试（`tests/assets.rs` 的 `unit_sprites_exist_square_and_keep_transparency`）
  因此可以只认「带 alpha 的方图」这一条，不必解析调色板；
- 将来换素材时按同一条规则处理即可（见 `bevy-assets` skill 的接入清单）。

## 字体：已子集化到 16 KB（8.3 MB → 16 KB）

**做法**：把「界面上真的会显示的字」抽出来，只保留这些字形的字体。
字符集有两个真相源，**都由脚本从源码里读**（不手抄）：

| 来源 | 怎么抽 |
| :--- | :--- |
| 英文 / 数字 / 标点 | `tests/assets.rs` 里 `ui_text` 那份数组（脚本直接解析它）**∪ 全部可打印 ASCII**（95 个） |
| 中文正文 | `chinese_in_source()`：从四个中文产处抽字符串字面量里的汉字，**跳过 `#[cfg(test)]` 模块** |

脚本在 [`tools/subset_font.py`](../tools/subset_font.py)：

```bash
# 1) 拉一份源字体（8.3 MB，**不要**提交进仓库）
#    https://cdn.jsdelivr.net/gh/googlefonts/noto-cjk@main/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf
# 2) 装工具（一次性）
python -m pip install fonttools
# 3) 子集化（从源字体切，不要从已子集化的文件再切）
python tools/subset_font.py <源字体.otf> assets/fonts/NotoSansSC-Regular.otf
```

⚠️ **别手抄字表**：第一版脚本手抄了 `ui_text`，漏掉 `n` / `B` / `g` 等 25 个字形，
而**资产测试照样绿**（它只对账中文那一段，ASCII 那一半没人查）——
屏幕上 `awaiting` / `COMBAT` 直接变成豆腐块。现在脚本从 `tests/assets.rs`
解析那份数组、并把整个可打印 ASCII 并进去，这类问题不会再发生。

**为什么能这么小**：界面上真正显示的 CJK 只有**战斗日志正文 + 威胁读数 + 溢出计数**
（HUD 文案是英文）——阵营标签 `玩家` / `敌人`、兜底 `单位`、
"命中 / 造成 / 受到 / 点伤害 / 阵亡 / 被击杀 / 还有 N 个"这些词。
当前 **154 个字形**：95 个可打印 ASCII + **43 个汉字** + 少量符号。

**加中文文案之后怎么办**：`cargo test --test assets` 会红（它逐字查 `cmap`），
按上面的脚本重新生成一次即可——**不要**把 8.3 MB 的原始字体提交回来。

**保留了什么 / 丢了什么**：保留这 154 个字形与 `cmap`；丢掉 `hinting`（屏幕字号下无所谓）、
`DSIG`（签名，子集化后本就失效）、以及不用的 OpenType 布局特性（本界面不做复杂排版）。
若将来需要**用户可输入**的文本（聊天 / 命名），就不能用子集字体，得换全量或按输入范围再扩。
