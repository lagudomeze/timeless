# Bevy 0.19 速查与最佳实践

> 版本基线：`0.19.1`（`Cargo.toml` 写 `bevy = "0.19"`，`Cargo.lock` 锁 0.19.1）。
> 本文只收录 0.19 的关键差异与易错点；完整用法以官方资料为准。
> 本仓库的 `bevy-019-docs` skill（`skills/bevy-019-docs`）保存了查证流程与来源优先级。

## 资料查询

权威来源优先级：Bevy GitHub（`examples/`、migration guides）→
[docs.rs/bevy/latest](https://docs.rs/bevy/latest/bevy/?search=) →
官方发布说明 / [bevy.org](https://bevy.org) →
[taintedcoders 0.19 TLDR](https://taintedcoders.com/bevy/tldr)（非官方，最佳实践补充）。

**禁止**用模型记忆充当权威；照搬 0.16 / 0.14 及更早的代码。

## 通信：Message 与 Event 分家

- `Message`：`#[derive(Message)]` + `app.add_message::<T>()`；
  `MessageWriter<T>` 写、`MessageReader<T>` 读；双缓冲存活 2 帧，
  不消费会被静默清理。
- `Event` / `EntityEvent`：配合 `GlobalTrigger` / `EntityTrigger` 与 `Observer`
  （首个参数为 `On<T, B>`）实现**即时、定向实体**的响应；
  触发用 `commands.trigger` / `trigger_targets`。
- `EventReader` / `EventWriter` 已移除；**Message 不会触发 Observer**，两者不可混用。

选型：批量广播、允许晚一帧 → Message；立即生效、针对具体实体 → Event + Observer。

## 场景与资产

- glTF 场景：`asset_server.load("models/x.glb#Scene0")` + `WorldAssetRoot(handle)`
  组件生成实体（替代旧 `SceneBundle`）。
- 批量加载：`asset_server.load_folder("textures/")`。
- 热重载：启用 `file_watcher` feature（dev profile）。
- 像素风：加载图片时用 `ImageSampler::nearest()`，或
  `DefaultPlugins.set(ImagePlugin::default_nearest())`。
- 精灵图集：`TextureAtlasLayout::from_grid(UVec2::splat(16), columns, rows)` +
  `Sprite { image, atlas }`；动画参考官方 `examples/2d/sprite_sheet.rs`。
- 场景书写：`bsn!` + `spawn_scene`，`Children [...]` 写子实体树，
  `asset_value(...)` 在场景里现场造材质。

## 组件、查询与关系

- `#[require(...)]` 声明必需组件，`#[derive(Bundle)]` 组合多个组件
  （0.19 没有手写 `Bundle` 的旧式 `derive` 语义，但命名 Bundle 仍然可用）。
- 组件钩子：`#[component(on_add = ...)]`（还有 `on_insert` / `on_remove` / `on_replace`）。
- 查询：`Single<T>` / `Populated<T>` 语义化单例与集合查询。
- 父子关系用 `ChildOf` / `Children`；两个查询都碰同一个组件时要写 `Without<...>`
  互斥，否则运行时 B0001。
- `Assets::get_mut` 返回 `AssetMut`（智能指针），绑定要 `mut` 才能解引用改写。

## UI 与文本

- UI 由 `Node` 布局驱动，不依赖 `Transform` 定位；
  `NodeBundle` / `TextBundle` / `Style` 已删除——UI 就是普通组件，字段并进 `Node`。
- 默认字体不含 CJK：中文界面必须自备字体资产（本仓库见 [assets.md](assets.md)）。
- `Timer` 自 0 向上 tick：全局计时放 `Resource`，随实体存亡的计时放 `Component`，
  单系统内部用 `Local<Timer>`。

## 输入

- 键盘 / 鼠标：`ButtonInput` 轮询状态；跨系统广播用 `MessageWriter` 发消息。
- 物理键 `KeyCode` 优先于逻辑键（布局无关、行为可预期）。
- 复杂重映射 / 上下文输入可评估 `bevy_enhanced_input`（选用前核对 0.19 兼容版本）。

## 调试（BRP）

项目启用了 `bevy_remote` 与 `bevy_brp_extras`，可以在运行时：

- `world.query` 按组件定位实体；**组件必须 `register_type` 才会出现在反射表里**，
  没注册的组件在远程协议里等于不存在（HUD 标记组件因此每个都要注册）。
- 读资源（`HoveredCell` 等注册过的资源）、模拟输入、截图、干净退出。

## 测试

- 逻辑测试用 `App::new()` + `MinimalPlugins`（+ 需要的领域 Plugin），不启动渲染。
- 整机夹具是 `crate::test_support::headless_app()`（`MinimalPlugins` + 领域插件 +
  `configure_pipeline`），**不存在** `AppPlugin` 这个类型。
- 单纯数据域（`world` / `voxel_render`）各自用 `MinimalPlugins` + 自己的 Plugin。

## 禁用 / 迁移清单

| 旧写法 | 0.19 写法 |
| :--- | :--- |
| `EventReader<T>` / `EventWriter<T>` | `MessageReader<T>` / `MessageWriter<T>`，或 Event + Observer |
| `SceneBundle` | `WorldAssetRoot` |
| `Camera2dBundle` / `Camera3dBundle` | `Camera2d` / `Camera3d` 组件 |
| `NodeBundle` / `TextBundle` / `Style` | `Node` / `Text` 等普通组件 |
| `app.add_state::<S>()` | `app.init_state::<S>()` |
