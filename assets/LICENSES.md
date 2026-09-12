# 素材许可清单（assets/）

> 逐项记录来源与许可证；新增素材必须在此登记。

| 路径 | 来源 | 许可证 | 备注 |
| :--- | :--- | :--- | :--- |
| `textures/ground/grass.png` | Kenney.nl Prototype Textures（`PNG/Green/texture_01.png`） | CC0 1.0 | https://kenney.nl/assets/prototype-textures |
| `models/nature/*.glb`（21 个） | Kenney.nl Nature Kit（`Models/GLTF format/*.glb`） | CC0 1.0 | https://kenney.nl/assets/nature-kit |
| `fonts/NotoSansSC-Regular.otf` | noto-cjk 仓库 `Sans/SubsetOTF/SC/NotoSansSC-Regular.otf` | OFL-1.1 | https://github.com/googlefonts/noto-cjk · 8.3 MB · 单一 Regular 字重 |

下载直链（zip 内 License.txt 亦随包提供）：

- https://kenney.nl/media/pages/assets/prototype-textures/a88c69fa18-1677578307/kenney_prototype-textures.zip
- https://kenney.nl/media/pages/assets/nature-kit/37ac38a37b-1677698939/kenney_nature-kit.zip
- https://cdn.jsdelivr.net/gh/googlefonts/noto-cjk@main/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf

CC0 1.0 无需署名，可自由商用与修改；OFL-1.1 允许自由使用、修改与再分发（修改后沿用 OFL，名称需避让保留字体名）。

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
