# Project Timeless — 进度与 backlog

> **本页只装「活的待办」与「怎么验收」。**
> 已完成的历史证据在 [`CHANGELOG.md`](CHANGELOG.md)（写完即冻结，别再往里加），
> 按领域分的条目在 [`docs/backlog/`](docs/backlog/)。
> 勾选规则：必须有验收证据（命令输出 / 测试名 / 控制台片段）才允许 `[x]`。

## 验收命令（仓库根目录）

```bash
cargo test                                  # 381 通过（378 单元 + 3 资产验收）/ 0 跳过
cargo clippy --all-targets -- -D warnings   # 零警告
cargo fmt --check
cargo run                                   # 冒烟：体素地形 + 世界空间战斗
```

实机冒烟清单（每轮提交前）：启动无 panic · 无资产加载错误 ·
决策 → 声明 → 前摇到点落地 → 扣血 → `F5` 重置可复现。

⚠️ **`cargo run` 与 `cargo test` 会互相抢 target 锁**：别并行跑，
否则先启动的那个进程会被替换掉（表现为"游戏自己退出了"）。

## 待办

条目正文按领域分在 `docs/backlog/`：**读哪块屏幕就翻哪个文件**。

> **2026-09-26 第二批已落地**（证据见各条目与 [`CHANGELOG.md`](CHANGELOG.md)）：
> `#47` 面板 `ready`/`busy` 读反 · `#50` 手动暂停状态行 · `#51` 格坐标多空格 ·
> `#52` Focus 读数 · `#53` 威胁两档读数 · `#54` 帮助的 CORE LOOP · `#55` 技能槽点击 ·
> `#56` 阵营环 · `#57` 威胁格 + 威胁来源圈 · `#60` 伤害数字 · `#61` 溢出计数 +
> 日志重叠 · `#62` 诊断锚点反射 · `#63` MCP 坑与核查清单 · 字体重打配方 ·
> 逻辑域与渲染分家。**下面表里剩下的都是还没做的。**

| 优先级 | 条目 | 在哪 |
| :--- | :--- | :--- |
| **P2** | **逻辑与表现分家**（Bevy 无头跑逻辑 + Godot 做表现与交互）：目标设计已成文，**未开工**、六处待拍板 | [`godot-client/`](docs/godot-client/README.md) |

**还没实机复跑的四条**（代码与测试都在，只差一张截图——列在这里是为了不让它们
靠"被遗忘"变成"已验证"）：伤害数字在冻结世界里的定格 · 威胁来源圈 ·
技能槽点击的金色描边 · 多敌人面板的「还有 N 个」（要 4 个敌人才看得到）。
逐条步骤见 [`docs/playtest-checklist.md`](docs/playtest-checklist.md) 第七节。

**主动不做、但留了触发条件的条目**（别当成遗漏）：
敌人面板可展开详情（读数多到一行放不下时再做）· 命中定帧 / 受击闪白
（先定"冻结时视觉纪律"表）· `PendingHit` / `ActionTemplate`
（出现多段技能或地面效果时再做，见 CHANGELOG 的审计结论）。

**还没拍板的**：命中定帧的幅度 · 「冻结时视觉纪律」表的成文位置。

## 当前代码是什么

`src/`（package `app`）= 12 个领域 + 组装车间：

- `world` 32³ 体素区块 / 噪声地形 / 体素读写 / 方块交互（零渲染依赖，`MinimalPlugins` 可单测）
- `voxel_render` 异步面剔除网格化（含贪婪合并 + 顶点 AO）/ 按类型分组材质 / 程序生成贴图
- `movement` `Cell` + `MoveGoal` 格子决策、`Transform` + `Velocity` 连续位移、
  移动 / 跳跃 / 翻滚 / 冲刺载荷与执行器、可行走性规则
- `combat` 生命 / 护甲公式 / 碰撞与近战扇形 / 攻击实体生命周期 / 箭矢与横扫 /
  火球锁格 + 真实距离 AoE / 精力 / 弹药 / 翻滚无敌帧 / 招架反制 / 格挡 / 反应槽
  （8 个子域，每个子域一个 `plugin.rs`，`CombatPlugin` 只编排顺序）
- `skills` **静态目录**：`AbilityId` / `AbilityDef` / `SkillRegistry` / `can_cast`
- `equipment` 槽位（`ChildOf` PC）/ 物品（`EquippedTo` 槽位）/ 类型校验 Observer /
  「基础值 + 加成」——有效值走纯函数（`armor_of` / `weapon_damage` / `weapon_timing`）
- `timeline` 无回合调度：`DecisionSlot`（`Idle { intent }` / `Executing { until }`）、
  行动归属（`ActionOf` / `Actions`）、每单位一份的 `Focus`、等待动作
- `clock` **通用冻结设施**：`PauseRequest` 每帧断言 + `ManualPause` 布尔，
  `process_pause_requests` 是**唯一的 `Time<Virtual>` 写入点**
- `ai` 六种战术（含威胁预判与花钱买前摇的闪避）
- `input` 只翻译（键位唯一真相在 `src/input/keyboard.rs`，玩家可见副本是 `HELP_LINES`）
- `interaction` 鼠标拾取 / 悬停高亮 / 预演 / 点击解释 / `PointerOverUi` 门控
- `presentation` 相机 / 单位纸片与贴地阴影 / 威胁格 / 命中粒子 / 中文日志 / 英文 HUD
- `spawn` 组装车间（消费 `ResetBattle`，不认识按键）

域地图与跨域契约见 [`docs/domain.md`](docs/domain.md)，文档入口是
[`docs/index.md`](docs/index.md)。

## 依赖与文档索引

> 本机 crates.io 直连不可用，版本经**中科大（USTC）镜像稀疏索引**查询确认
> （配置在 `~/.cargo/config.toml`）。
> 依赖一律手动写入 `Cargo.toml`（`cargo add` 在镜像下不可用）；
> 新依赖确认版本后先登记下表再引入。
> ⚠️ 本地索引缓存**只在改过 `Cargo.toml` 后**才刷新：解析失败先改一次再试，
> 别急着判定"镜像没有这个版本"。

| 依赖 | 版本 | 用途 |
| :--- | :--- | :--- |
| bevy | 0.19.1 | 引擎（`Cargo.toml` 写 `0.19`，`Cargo.lock` 锁 0.19.1） |
| rand | 0.10.2 | 装饰物随机摆放 |
| bevy_brp_extras | 0.22 | 运行时调试协议扩展：截图 / 输入模拟 / 干净退出 |
| serde + ron | 已引入 | `config/actions.ron` 数值配置的序列化 |
| notify-debouncer-full | 0.7.0 | `hot-reload` feature：`config/actions.ron` 的热重载 |

引擎官方文档：<https://bevy.org/learn/> ·
迁移指南：<https://bevy.org/learn/migration-guides/> ·
符号检索：<https://docs.rs/bevy/latest/bevy/?search=>

## 开发规范

完整规范见 [`AGENTS.md`](AGENTS.md)；跨域契约与十二条铁律见
[`docs/domain.md`](docs/domain.md) 第五节。要点：

1. **分层解耦**：`combat/formula/domain.rs` 零 Bevy 依赖；应用层不含伤害公式。
2. **数据驱动**：数值 / 技能走外部配置（`config/actions.ron`），应用层不硬编码。
3. **消息通信**：模块间用 Bevy `Message`；组件 / 消息 / 系统同属一个领域文件；
   UI 输入只翻译成消息；消息在**消费方**插件注册并注明谁写谁消费。
4. **文档先行**：用 Bevy API 前先查 docs.rs / 官方示例（`bevy-019-docs` skill）。
5. **文档防漂移**：进度只写本文件与 `CHANGELOG.md`；文档里引用的类型名必须先在
   `src/` 里 grep 确认存在。
