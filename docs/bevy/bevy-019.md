# Bevy 0.19 速查与最佳实践

> 版本基线：`0.19.1`。本文只收录 0.19 的关键差异与易错点；完整用法以官方资料为准（查询规范见 `bevy-019-docs` skill）。

## 资料查询

权威来源优先级：Bevy GitHub（examples、migration guides）→ [docs.rs/bevy/latest](https://docs.rs/bevy/latest/bevy/?search=) → 官方发布说明 → [taintedcoders 0.19 TLDR](https://taintedcoders.com/bevy/tldr)（非官方，作为最佳实践补充）。

## 通信：Message 与 Event 分家

- `Message`：`#[derive(Message)]` + `app.add_message::<T>()`；`MessageWriter<T>` 写入、`MessageReader<T>` 读取；双缓冲存活 2 帧。
- `Event`（全局）/ `EntityEvent`（实体）：配合 `GlobalTrigger` / `EntityTrigger` 与 `Observer`（首个参数为 `On<T, B>`）实现**即时**行为；触发用 `commands.trigger` / `trigger_targets`。
- `EventReader` / `EventWriter` 已移除；`Message` 不会触发 `Observer`。

## 场景与资产

- glTF 场景：`asset_server.load("models/x.glb#Scene0")`，用 `WorldAssetRoot(handle)` 组件生成实体（替代旧 `SceneBundle`）。
- 批量加载：`asset_server.load_folder("textures/")`。
- 热重载：启用 `file_watcher` feature（开发 profile 可用 `dev` 特性集合）。
- 像素风：`DefaultPlugins.set(ImagePlugin::default_nearest())` 避免低清贴图被线性过滤糊掉。
- 精灵图集：`TextureAtlasLayout::from_grid(UVec2::splat(16), columns, rows)` + `Sprite { image, atlas }`；动画参考官方 `examples/2d/sprite_sheet.rs`。

## 组件、查询与场景标记

- `#[require(...)]` 声明必需组件，自动补齐。
- 组件钩子：`#[component(on_add = ...)]`（还有 `on_insert` / `on_remove` / `on_replace` 等）。
- 查询：`Single<T>` / `Populated<T>` 语义化单例与集合查询。
- `Relationship`：实体间关系组件（层级、拥有等），BSN 中用 `bsn!` 书写场景模板。

## 输入

- 键盘 / 鼠标：`ButtonInput` 轮询状态，跨系统广播用 `MessageWriter` 发消息。
- 物理键 `KeyCode` 优先于逻辑键（布局无关、行为可预期）。
- 复杂重映射/上下文输入：可评估 `bevy_enhanced_input`（选用时核对 Bevy 0.19 的兼容版本表）。

## UI 与计时

- UI 由 `Node` 布局驱动，不依赖 `Transform` 定位。
- 默认字体不含 CJK，中文界面需自备字体资产。
- `Timer` 自 0 向上 tick：全局计时放 `Resource`，技能 CD 放 `Component`，单系统内用 `Local<Timer>`。

## 测试

- 逻辑测试用 `App::new()` + `MinimalPlugins`（+ 需要的领域 Plugin），不启动完整渲染。
  代码 A 的整机夹具是 `crate::test_support::headless_app()`；**没有** `AppPlugin` 这个类型。

## 禁用 / 迁移清单

| 旧写法 | 0.19 写法 |
| :--- | :--- |
| `EventReader<T>` / `EventWriter<T>` | `MessageReader<T>` / `MessageWriter<T>`，或 Event + Observer |
| `SceneBundle` | `WorldAssetRoot` |
| `Camera2dBundle` / `Camera3dBundle` | `Camera2d` / `Camera3d` 组件 |
| `app.add_state::<S>()` | `app.init_state::<S>()` |

遇到对话记录或旧教程中的其他 API，先查证再使用，禁止照搬。
