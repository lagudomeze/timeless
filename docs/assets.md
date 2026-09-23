# 素材与字体

> **描述对象：仓库 `assets/` 与实际引用它的代码。**
> 许可与体积的逐项记录在 [`assets/LICENSES.md`](../assets/LICENSES.md)；
> 本文是选源原则与接入方式的速览。
> 完整流程（下载、许可检查、接入步骤）见 `skills/bevy-assets`。

## 许可原则

| 许可证 | 处理 |
| :--- | :--- |
| **CC0** | 首选：可自由商用、无需署名 |
| **CC-BY** | 可用，必须保留署名清单（入库时记录来源与作者） |
| **GPL** | 有传染性，商业化前评估风险，尽量避开 |

入库时在 `assets/LICENSES.md` 里逐项记录「素材路径、来源、许可证、署名要求」。

## 当前素材清单

| 类别 | 内容 | 许可 |
| :--- | :--- | :--- |
| 单位精灵 | `textures/units/{player,enemy,shadow}.png` | Kenney Tiny Dungeon（CC0） |
| 技能图标 | `textures/ui/icon_{attack,melee,fireball,roll}.png` | 同上 |
| 地表装饰 | `models/nature/*.glb`（21 个：树 / 石 / 草 / 花 / 木桩…） | Kenney Nature Kit（CC0） |
| HUD 字体 | `fonts/NotoSansSC-Regular.otf`（OFL-1.1，8.3 MB） | OFL-1.1 |
| 未引用 | `textures/ground/grass.png` | 见 `assets/LICENSES.md` |

字体体积取舍与子集化出路见 `assets/LICENSES.md`。当前 8.3 MB 全覆盖；
子集化可缩到几十 KB，但要在「加新文案不能缺字」和「仓库体积」之间取舍。

## 字体：为什么必须自带

Bevy 默认字体**不含 CJK**，而战斗日志正文是中文（`presentation/log.rs`）。
因此 HUD 的所有文本都显式指定 `HUD_FONT`（`assets/fonts/NotoSansSC-Regular.otf`），
而不是用 `TextFont::default()`。

两条纪律：

1. **改日志文案必须跑 `tests/assets.rs`**——它读真实字节验字体文件是合法 sfnt；
   逐字查 `cmap` 需要第三方解析库（已移除），字形覆盖目前靠运行时人工确认
   （见 [../TODO.md](../TODO.md)）。
2. **加技能要加图标**：`SKILLS` 里每加一条，`icon_path(kind)` 指到的 PNG 都会被
   `tests/assets.rs` 要求存在、是带 alpha 的方图。

### 已知问题：CJK 断行

Bevy 文本栈缺 `icu_segmenter` 的 CJK 分词模型，运行时会打印
`ICU4X data error: No segmentation model for complex script`——
正文仍正常渲染，只是断行退化。要彻底解决需要引入分词模型（后续项）。

## 选源清单（2D / 3D）

**2D**

- **Kenney.nl**：CC0 全集，风格统一、面向游戏，Bevy 官方示例同款（当前在用）。
- **OpenGameArt.org**：社区量大，逐项检查许可。
- **Itch.io（Game Assets）**：筛选 Free / PWYW。
- **Universal LPC Spritesheet Generator**：在线生成角色精灵表（含行走、攻击动画），
  注意其许可的署名条款。

**3D**

- **Poly Haven**：CC0 PBR 纹理与 HDR。
- **ShareTextures**：4K PBR 纹理。
- **Hyper3D / TerraForge3D**：地形模型与程序化地形。
- **Mixamo**：免费角色与动作，导出 FBX 后经 Blender 转 glTF。

## 伪 3D 方案（当前采用）

**3D 场景 + 2D 纸片（Billboard）+ 贴地阴影**：树木 / 石头用 3D 模型；
角色与特效用始终面向摄像机的 2D 精灵，脚下放半透明贴地阴影表示高度。

- 优势：成本低一个量级、漂浮物识别度高、风格化复古感；
- 取舍：光影画死（无实时明暗）、纸片无厚度（镜头侧转穿帮）、无体积遮挡；
- 实现：`presentation/unit_sprite.rs`（`billboard_system` + `shadow_system`），
  单位根节点保持「脚底 + 无旋转 + 无缩放」（纸片与阴影是它的子节点，靠局部坐标
  直接表达世界偏移；见 [relations.md](relations.md) 的「物理附着用 `ChildOf`」）。

## 接入 Bevy 0.19 的要点

- 像素风：加载时用 `ImageSampler::nearest()`（`load_builder().with_settings(..)`），
  避免低清贴图被线性过滤糊掉；
- glTF 场景：`assets/models/x.glb#Scene0` + `WorldAssetRoot` 组件
  （替代旧 `SceneBundle`），装饰表见 `presentation/decoration.rs`；
- 热重载与批量加载：`load_folder`、`file_watcher`（后续项）；
- 中文字体：见上文——默认字体无 CJK，必须自备。
