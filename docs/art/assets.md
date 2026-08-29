# 免费素材获取与接入

> 本页是选源与许可的速览；完整流程与 Bevy 接入步骤见 `bevy-assets` skill（源码在 `skills/bevy-assets`）。

## 许可原则

- **CC0**：可自由商用、无需署名，首选。
- **CC-BY**：可用，但必须保留署名清单（入库时记录来源与作者）。
- **GPL**：有传染性，商业化前评估风险，尽量避开。
- 入库时在 `assets/` 内附 `LICENSES.md`，逐项记录「素材路径、来源、许可证、署名要求」。

## 2D 素材来源

- **Kenney.nl**：CC0 全集，风格统一、面向游戏，Bevy 官方示例同款。
- **OpenGameArt.org**：社区量大，逐项检查许可。
- **Itch.io（Game Assets）**：筛选 Free / PWYW。
- **Generic RPG Pack**：CC0 通用 RPG 包（Bevy 官方文档引用）。
- **Universal LPC Spritesheet Generator / AIPixelKit**：在线生成角色精灵表（发型/服装/武器组合，含行走、攻击动画）；注意其开源许可的署名条款。

## 3D 素材来源

- **Hyper3D**：免费地形模型（GLB / OBJ / FBX）。
- **Poly Haven**：CC0 PBR 纹理与 HDR。
- **ShareTextures**：4K PBR 纹理。
- **TerraForge3D**：程序化地形。
- **Mixamo**：免费角色与动作，导出 FBX 后经 Blender 转 glTF 供 Bevy 使用。

## 伪 3D（推荐方案）

**3D 场景 + 2D 纸片（Billboard）+ 贴地阴影**：场景中的树木、石头用 3D 模型；角色、道具、特效用始终面向摄像机的 2D 精灵，脚下放半透明贴地阴影。

- 优势：成本低一个量级、漂浮物识别度高、风格化复古感。
- 取舍：光影画死（无实时明暗）、纸片无厚度（镜头侧转穿帮）、无体积遮挡。
- 技巧：漂浮物绕 Y 轴旋转可脑补「厚度」；利用场景体积雾/全局光提升融合度。

## 接入 Bevy 0.19

关键点（详细步骤与示例见 skill 的 `references/bevy-integration.md`）：

- 像素风：`ImagePlugin::default_nearest()`。
- 精灵图集：`TextureAtlasLayout::from_grid`。
- 3D 场景：`assets/x.glb#Scene0` + `WorldAssetRoot`。
- 批量与热重载：`load_folder`、`file_watcher`。
- 中文字体：默认字体无 CJK，需自备字体资产。
