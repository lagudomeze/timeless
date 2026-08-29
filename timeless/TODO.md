# Project Timeless — 项目管理 TODO

> 更新于 workspace 建立日。勾选状态以会话内 todo 为准，本文件是持久化版本。

## 目录结构

```
timeless/                     # cargo workspace 根（子项目，不在仓库根目录）
├── Cargo.toml                # workspace 成员声明
├── vendor/parley/            # 本地补丁：parley 0.9.0（CJK 词典分词），见 README.patch.md
├── docs/
│   └── timeline-core-design.md   # 时间线核心设计文档 v0.1（接口签名版）
└── crates/
    ├── timeless-domain/      # 领域层：纯 Rust，零 Bevy 依赖（可单测）
    │   ├── src/combat.rs     # 「速度帧→距离→破势」三层裁决 + 7 测试
    │   └── src/grid.rs       # GridPos 距离/移动 + 4 测试
    └── timeless-app/         # 应用层：Bevy 组件 / 系统 / 渲染
        ├── src/combat.rs     # 战斗领域：Health/Damage、攻击属性、意图/状态组件 + 结算系统
        ├── src/movement.rs   # 移动领域：Position、位移意图、投射物（火球）组件与系统
        ├── src/timeline.rs   # 时间线领域：阶段机 + 重置消息/系统
        ├── src/menu.rs       # 菜单领域：Action（UI 选项）/游标/反应选项 + 输入系统
        ├── src/display/      # 展示层子域：camera / unit（纸片+阴影）/ hints（箭头）/ hud / map
        ├── src/setup.rs      # 场景搭建
        ├── src/debug.rs      # 调试面板
        └── src/main.rs       # 入口
```

## 开发规范（来自架构原则）

1. **分层解耦**：领域层（`timeless-domain`）绝不引入 `bevy`；应用层不含伤害公式。
2. **数据驱动**：技能/怪物/标签反应走外部配置（Phase 2，serde+ron）。
3. **消息通信**：模块间用 Bevy `Message`（`ActionSubmitted` / `HitLanded` / `RollExecuted` 等，
   与所属领域的组件/系统放在同一文件）。
4. **文档先行**：每个依赖使用前，先查下表 docs.rs 对应版本的 API。

## 依赖与文档索引（crates.io 检索结果）

> 检索方式：本机 crates.io 直连被网络阻断，版本经 **清华镜像稀疏索引** 查询确认
> （cargo 缓存索引 `~/.cargo/registry/index/mirrors.tuna.tsinghua.edu.cn-*/.cache/`）。

| 依赖 | 版本（最新稳定） | crates.io | docs.rs | 用途 / 引入阶段 |
| :--- | :--- | :--- | :--- | :--- |
| bevy | 0.19.1 | https://crates.io/crates/bevy | https://docs.rs/bevy/0.19.1 | 引擎（已引入，Phase 1） |
| bevy_inspector_egui | 0.37.0（已引入） | https://crates.io/crates/bevy-inspector-egui | https://docs.rs/bevy-inspector-egui/0.37.0 | egui 依赖（0.37 配 bevy 0.19，0.36 配 0.18）；面板已改为自研极简版 |
| bevy_egui | 0.40.1（已引入） | https://crates.io/crates/bevy_egui | https://docs.rs/bevy_egui/0.40.1 | 调试面板运行时（直接依赖方可 use） |
| serde | 1.0.229 | https://crates.io/crates/serde | https://docs.rs/serde/1.0.229 | 配置序列化（Phase 2） |
| ron | 0.12.2 | https://crates.io/crates/ron | https://docs.rs/ron/0.12.2 | .ron 配置格式（Phase 2） |

引擎官方文档：https://bevy.org/learn/ · 迁移指南（0.17→0.18：https://bevy.org/learn/migration-guides/0-17-to-0-18/ · 0.18→0.19：https://bevy.org/learn/migration-guides/0-18-to-0-19/）

> 版本来源注：首次以本地缓存索引判断为 0.17.3，cargo 刷新索引后发现最新稳定版为 0.19.1，
> 已更正并切换。**判断依赖最新版本以 cargo 解析输出（`Adding X (available: Y)`）为准。**

⚠️ 环境注意事项：
- 依赖一律写在 `Cargo.toml` 后由 `cargo build` 经清华镜像解析（`cargo add` 在源替换
  配置下会去查询被替换的 crates.io 而失败——已知兼容性问题，勿用）。
- `cargo search` 仅支持 crates.io 官方搜索 API，镜像下不可用；查版本用上述缓存索引法。
- 新依赖版本确认后，先更新本表再引入代码。

## 里程碑

### Phase 1.5 — 伪 3D 场景（3D 场景 + 2D 纸片）✅

- [x] 素材选源与入库：**Kenney.nl**（CC0）—— Prototype Textures 草地贴图 + Nature Kit 低模 GLB（自包含、无外部贴图），`assets/LICENSES.md` 逐项登记
- [x] 地图：21×21 格（441 块草地贴图，深色底板留缝形成网格线）
- [x] 装饰：73 个低模 GLB（树/石/灌木/花/草），固定种子 LCG 保证布局一致、出生点留空
- [x] 单位：改为 2D 纸片（Rectangle + unlit 材质 + `Billboard` 系统朝向相机）+ 贴地阴影
- [x] 资产根目录：`AssetPlugin.file_path` 固定为编译期 `CARGO_MANIFEST_DIR/assets`（cargo run 与直接运行 exe 均可用）
- [x] 相机平移/缩放：右键按住拖动平移（egui 调试面板外生效）+ 滚轮缩放 FOV（键盘不再参与镜头移动）
- [x] 菜单交互：Tab/Shift+Tab 循环切换行动（攻击/移动/翻滚，移除 A 键与方向键导航），空格确认/提交
- [x] 移动行动：WASD/方向键生成「移动」目标（支持斜向——同按或先后按会叠加方向；钳制网格内），结算时位移；无目标时空提交被拦截
- [x] 移动箭头：排定 Move 目标时用内置 Gizmos 绘制黑色方向箭头（即时模式，随意图出现/消失）
- [x] 阴影跟随：单位改为「根节点 + 纸片/阴影子实体」层级（根节点不旋转，重置级联清理）
- [x] 中文 HUD：NotoSansSC-Regular.otf（OFL-1.1，`assets/LICENSES.md` 已登记）接入
- [x] CJK 分词修复：`vendor/parley` 补丁启用 `WordSegmenter::new_auto`，消除 ICU4X "No segmentation model" 刷屏（20s 运行 0 报错）
- [x] 验收：`cargo build` / `clippy` 零警告 / `fmt --check` 通过；实机运行 22s 无资产错误
- [ ] 纸片角色贴图（带 alpha 的 PNG 角色，替换纯色占位；Kenney 角色包或 LPC 生成器）
- [ ] 单位头顶状态回 3D（Text2d 在 Camera3d 下不渲染；需 2D 叠加相机或世界→屏幕投影）
- [ ] 开发热重载：`file_watcher` feature（dev profile）

### Phase 1.6 — 应用层模块化重构 ✅

- [x] `display` 拆成 mod 目录：`camera` / `unit`（纸片+贴地阴影）/ `hints`（移动箭头）/
      `hud` / `map`（地面+装饰+网格常量），`setup` 只做组合
- [x] 移动与战斗分域：`movement.rs` 承载 `Position`、位移意图与投射物
- [x] `AttackStats` 拆为 `AttackFrame` / `AttackRange` / `Impact` / `Damage` 四个小组件
      （领域层 `AttackStats` 保留为纯函数入参，裁决前组装）
- [x] `ActionQueue` / `Action` 降级为 UI 层选项；回合行动改为意图组件
      （`AttackIntent` / `MoveIntent` / `RetreatIntent`），结算后清除
- [x] 闪躲/招架/打断组件化：`DodgeActive` / `Parry`（预留）/ `Interrupted`
- [x] 火球投射物：`Projectile` + `Destination` + `ExplosionDamage` 组件组合，
      每帧推进、到达后范围伤害；调试面板新增 Fireball 测试按钮
- [x] 伤害应用与死亡检查拆成独立系统（`resolve_system` → `projectile_system` →
      `death_check_system`），火球任意阶段击杀也能正确结束战斗
- [x] 设计文档 `docs/design/ecs-combat-components.md`（含社区参考）
- [x] 验收：`cargo test --workspace` 全绿 / `clippy -D warnings` 零警告 /
      `fmt --check` 通过 / `cargo build` 成功

### Phase 0 — 项目搭建 ✅
- [x] 检索依赖版本（bevy / bevy_inspector_egui / serde / ron）
- [x] cargo workspace 子项目（`timeless/`，不在根目录）
- [x] 领域层/应用层 crate 拆分，Part 1/2 代码迁移
- [x] `cargo test` 领域层 11 个测试通过

### Phase 1 — 纵向切片（首个可反馈原型，5 分钟内展现核心博弈）✅
- [x] `events.rs`：`ActionSubmitted` / `HitLanded` / `RollExecuted` / `ResetBattle`（bevy 0.19 Message 体系）
- [x] `input_system`：A 切换行动（Attack ⇄ Roll）、Space 提交（决策暂停门控）
- [x] `ai_system`：敌人移动一格 / 进入射程锁定攻击（攻击帧 5），每回合仅决策一次
- [x] `time_advance_system`：收集 ActionQueue → 调用领域层裁决 → 广播消息
- [x] `roll_cancel_system`：Q 键前摇窗口取消（精力×2）
- [x] 渲染：5x5 网格 + 玩家蓝块 + 敌人红块 + HUD（英文，默认字体无 CJK）
- [x] 验收：控制台「谁先命中」✅ / 重置按钮+R 键 ✅（实战验证）
- [x] 调试：自研极简 egui 面板（Combat Debug），替代庞杂 WorldInspector
- [ ] T5 翻滚取消实战验证（Q 键 0.5s 窗口 + 精力不足分支）
- [x] CJK 中文字体资产（HUD 中文显示，Phase 1.5 已完成，见上）

### Phase 2 — 数据驱动 + 时间线核心（设计文档 v0.1）
- [ ] serde + ron 加载 ActionDef 配置（assets/*.ron）
- [ ] ActionDef 注册表（phases / cost / cooldown / cancelable_by / impact）
- [ ] Phase 推进系统 + `try_cancel` + `ReactionOpportunityEvent` + 决策暂停状态机

### Phase 3 — 策略深度（二/三梯队）
- [ ] 格挡/招架（CancelRule by GUARD）、范围攻击（L2 排程）、冲刺
- [ ] G 层：洞察力查看怪物数据、战斗日志、死亡复盘
- [ ] 建造/后勤：采集、弹药制造、工事（接口预留）

## 验收标准（每次提交前）

- [ ] `cargo test --workspace` 全绿
- [ ] `cargo clippy --workspace` 无警告
- [ ] `cargo fmt --check` 通过
