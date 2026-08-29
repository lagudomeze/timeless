---
name: bevy-assets
description: 为 Bevy 0.19 项目获取免费 2D/3D/音频素材并接入引擎：选源、许可证检查、assets 目录组织、像素配置、精灵图集、glTF 场景（WorldAssetRoot）与热重载，含伪 3D（3D 场景 + 2D 纸片）方案。不用于生成原创美术。
---

# Bevy 免费素材获取与接入

## 何时使用

需要为游戏寻找免费素材、把素材接入 Bevy、或审查素材入库（许可 / 目录 / 加载方式）时使用。

## 工作流

1. 明确需求：风格（像素 / 低模）、类型（角色 / 地图 / 特效 / UI / 音频）、格式约束（glTF / PNG）、许可底线。
2. 选源：2D 看 [references/2d-sources.md](references/2d-sources.md)；3D 与伪 3D 看 [references/3d-sources.md](references/3d-sources.md)。
3. 许可证检查：CC0 首选；CC-BY 记署名；GPL 慎用；AI 生成素材确认平台条款。下载时即记录来源 URL。
4. 组织目录：`assets/` 下按 `textures/`、`models/`、`audio/`、`fonts/` 分类，并附 `LICENSES.md` 逐项记录来源、许可证、署名要求。
5. 接入 Bevy 0.19：按 [references/bevy-integration.md](references/bevy-integration.md) 的清单执行。

## 关键决策

- 像素风：一律 `ImagePlugin::default_nearest()`，否则贴图被线性过滤糊化。
- 2D 动画：优先选择已带精灵表（含 idle / run / attack 帧）的素材，用 `TextureAtlasLayout` 组织。
- 3D：glTF（.glb）优先；Mixamo 的 FBX 需经 Blender 转 glTF。
- 伪 3D（低成本推荐）：3D 场景 + Billboard 纸片 + 贴地阴影，详见 3d-sources.md。

## 参考资料

- 2D 素材来源： [references/2d-sources.md](references/2d-sources.md)
- 3D 素材与伪 3D 方案： [references/3d-sources.md](references/3d-sources.md)
- 接入 Bevy 0.19 步骤： [references/bevy-integration.md](references/bevy-integration.md)
