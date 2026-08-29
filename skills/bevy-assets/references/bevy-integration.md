# 接入 Bevy 0.19

> API 以 bevy 0.19 为准；符号有疑问用 `bevy-019-docs` skill 查证。

## 1. 像素与贴图

- 装配时：`DefaultPlugins.set(ImagePlugin::default_nearest())`，禁用线性过滤。
- 单张图片：`asset_server.load("textures/char.png")` + `Sprite { image, .. }` 组件。

## 2. 精灵表动画

- `TextureAtlasLayout::from_grid(UVec2::splat(16), cols, rows)` 生成布局并 `assets.add(layout)`。
- Sprite 指定 `image` 与 `texture_atlas`，逐帧切换 atlas 索引；参考官方 `examples/2d/sprite_sheet.rs`。

## 3. 3D 场景（glTF）

- 预加载：`asset_server.load("models/x.glb#Scene0")`，把 Handle 存入资源。
- 生成实体：`commands.spawn(WorldAssetRoot(handle))` + `Transform`。
- 材质覆盖、动画等扩展：查官方 `examples/gltf` 目录。

## 4. 批量加载与热重载

- `asset_server.load_folder("models/")` 批量取 Handle。
- 开发时启用 `file_watcher` feature 实现热重载（dev profile 可开 `dev` 特性集合）。

## 5. 目录与许可

- `assets/` 下分 `textures/`、`models/`、`audio/`、`fonts/`；新建 `assets/LICENSES.md` 记录每项来源、许可证、署名要求。
- 音频：Kenney 音效（CC0）；字体：CJK 需自备（Bevy 默认字体无中文）。

## 6. 验收

- 通过资源加载状态确认资产就绪后再生成实体，避免首帧引用未加载资源；加载失败时打印错误并记录来源，便于回退替换。
