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
| `fonts/NotoSansSC-Regular.otf` | noto-cjk 仓库 `Sans/SubsetOTF/SC/NotoSansSC-Regular.otf` | OFL-1.1 | https://github.com/googlefonts/noto-cjk · 8.3 MB · 单一 Regular 字重 |

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

## 字体为什么这么大

HUD 里会出现的 CJK 文本只有**战斗日志正文**
（`presentation/log.rs`：`"{who} 受到 {n} 点{type}伤害"`、`"{who} 阵亡"`、
阵营标签 `玩家` / `敌人`、找不到阵营时的兜底标签 `单位`）——实际用到的汉字不到 20 个。
但现成字体是「简体常用字全覆盖」，无法只带那几个字，除非自己做子集化。
当前取舍是**可读性优先**（8.3 MB）。

两条守住这件事的机制：

1. `tests/assets.rs` 会读真实字体逐个查 `cmap`，确认日志能输出的每个字都有字形。
   **改日志文案时若引入字体没覆盖的字，测试直接失败**——否则运行时只会静默变成豆腐块。
2. 若将来在意体积，把用到的字符子集化成一个几十 KB 的小字体即可，
   上面的测试会照旧通过（它只关心覆盖，不关心体积）。
