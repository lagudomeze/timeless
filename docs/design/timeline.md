# 时间线系统详细设计

> 状态：设计定稿 v0.2。本文取代 [timeline-core-design.md](timeline-core-design.md)（v0.1）中与本文冲突的约定（如 `ActionId(&'static str)`、旧 `Event` 用法、DecisionPause 冻结模式）。
> 关联：[game-design.md](game-design.md)（动机）、[architecture.md](architecture.md)（分层与通信）、[../bevy/action-graph.md](../bevy/action-graph.md)（行为描述）、[../bevy/bevy-019.md](../bevy/bevy-019.md)（Bevy 0.19 速查）。

> ⚠️ 已过时：自 Phase 1.13 起，代码实现改为**无回合**模型——`Time<Virtual>`
> 持续流动驱动动作调度，不再有 Planning / Resolving 阶段状态机与逻辑刻度跳跃。
> 本文的 We-Go 阶段机、`CombatTimeline` 逻辑刻度等约定待重写；动作实体 +
> `Declared → Pending → Committed` + 两阶段结算与三层裁决仍有效。

## 1. 目标与范围

把 **We-Go 同步回合**落实为可实现的领域模型与 Bevy 0.19 映射，覆盖：两阶段循环、逻辑刻度时间模型、执行队列与三层裁决、时间线特权（无限撤销 / 翻滚取消）、怪物意图循环、延迟命中（`PendingHit`）、UI 信息需求。

不在本文范围：数值平衡、AI 策略细节（留待未决问题）。

## 2. 时间模型：逻辑刻度，不是真实时间

战斗时间由两层构成：

| 层 | 名称 | 作用 | 驱动 |
| :--- | :--- | :--- | :--- |
| L1 | 行动管线 `ActionPipeline` | 每个实体当前动作的 前摇→判定帧→后摇 | Entity Component |
| L2 | 全局时间线 `CombatTimeline` | 未来事件（弹道、延迟 AOE、地面效果）排程 | Resource |

**帧（frame）是逻辑刻度**：结算器中的指针 / 索引，由 `Resolving` 队列驱动，**不是 Bevy 的 `Time` 资源**。

- 决策阶段（Planning）：帧表现为**静态窗口坐标**——动作的判定帧在时间轴上的位置，用于判断「当前是否在前摇窗口内」。
- 执行阶段（Resolving）：帧只作为**排序权重**——谁的判定帧先到，谁先命中。

`GlobalTime` 跳跃式推进：每轮执行结束后 `GlobalTime += 本轮所有行动消耗的最大刻度数`。速度快的单位靠「下次行动间隔短」获得更高行动频率，而不是靠轮次特权。

**为什么不用真实时间**：确定性、可撤销、可回放、领域层可单测；与表现层动画（可后补）彻底解耦。

## 3. 阶段状态机

```
Planning ──(全部就绪/玩家确认)──▶ Resolving
   ▲                                │
   └────────(执行完毕)──────────────┘
```

| 阶段 | 规则 |
| :--- | :--- |
| **Planning（决策）** | 所有单位同时提交草案（`PendingDraft`）；玩家可无限覆盖草案（撤销=覆盖，无惩罚）；AI 提交后锁定。 |
| **Resolving（执行）** | 收集全部草案 → 构建执行队列 → 按逻辑刻度逐帧推进 → 命中帧校验 → 应用结果 → 推进 `GlobalTime` → 清空草案。 |

威胁提示采用「前摇窗口」表达，**不默认冻结时间线**（v0.1 的 `DecisionPause` 冻结降级为可选项，见未决问题）：玩家在执行阶段的前摇窗口内按下翻滚即实时生效，与怪物攻击的命中帧做时空交错校验。

## 4. 数据结构（领域层，纯 Rust，零 Bevy 依赖）

```rust
/// 规划阶段草案：可以覆盖，直到进入执行阶段
pub struct PendingDraft {
    pub actor: EntityId,
    pub action: ActionRef,          // usize 索引 或 Handle<ActionTemplate>，不用 String ID
    pub target: Option<EntityId>,
    pub dir: Option<GridDir>,
}

/// 执行阶段锁定的行动：Priority 越小越先结算
pub struct ScheduledAction {
    pub draft: PendingDraft,
    pub hit_clock: u32,             // 判定帧所在的逻辑刻度
    pub distance: u32,              // 命中距离（第二层裁决）
    pub poise: u32,                 // 破势（第三层裁决）
}

/// 执行队列：按 (hit_clock, distance, poise, actor_id) 升序
pub struct ExecutionQueue {
    pub scheduled: Vec<ScheduledAction>,
    pub cursor: u32,                // 当前逻辑刻度指针
}

/// L2 全局时间线：未来事件排程（弹道飞行、延迟 AOE、地面效果）
pub struct CombatTimeline {
    pub clock: u32,
    pub scheduled: BinaryHeap<ScheduleEntry>,   // 按 at 升序
}

/// 延迟命中物化为实体（不依赖回调/异步）
pub struct PendingHit {
    pub origin: EntityId,
    pub target: EntityId,
    pub hit_at_clock: u32,
    pub damage: i32,
    pub element: Option<Element>,
    pub state: HitState,            // Flying / Landed / Whiffed / Dodged
}

/// 玩家独有特权守卫：仅玩家实体携带
pub struct CancelPrivilege;
```

资源组件：`Stamina`（精力）、`AmmoPouch`（弹药）、`Cooldowns`（技能冷却）、`Poise`（架势槽）。

## 5. 核心算法

### 5.1 执行队列构建与三层裁决

1. 收集所有 `PendingDraft`（玩家 + AI）。
2. 由 `ActionTemplate`（phases / hit_frame / speed / cost）计算每个行动的 `hit_clock = 当前刻度 + startup 刻度`。
3. 排序键：**hit_clock → distance → poise → actor_id**（数值小者先结算）。
4. 对砍情形：先结算者的伤害先生效；若先结算者击杀目标，后者的行动**取消**（目标已死亡）。

### 5.2 时间推进

`Resolving` 循环：指针推进到下一个有事件或行动的刻度 → 触发该刻度全部事件 → 校验命中 → 应用效果 → 直到队列与 `CombatTimeline` 都为空。全部完成后 `GlobalTime += 本轮最大行动消耗`，切回 Planning。

### 5.3 命中帧校验

命中帧触发时，基于**当时**的目标坐标与状态判定：坐标在攻击范围内且不在无敌帧 → 命中；目标已位移 → 未命中（怪物攻击不取消，只是打空，进入后摇）；目标处于格挡 → 减伤并消耗架势。

### 5.4 翻滚取消（玩家时间线特权）

攻击动作三段式窗口：

| 阶段 | 占比（示例） | 可操作性 |
| :--- | :--- | :--- |
| ① 前摇 Wind-up | 0~60% | **可翻滚取消**（弹药已扣，伤害未产生） |
| ② 判定帧 Hit Frame | 60~70% | 不可取消，伤害在此结算 |
| ③ 后摇 Recovery | 70~100% | 不可取消 |

规则：

- 仅阶段①内按下翻滚生效；判定帧/后摇按翻滚 = 无效 + 翻滚短暂 CD 惩罚（防狂按）。
- 效果：中断当前动作（**弹药不返还**）→ 位移 1~2 格 + 无敌帧（约 0.5s/若干刻度）→ 翻滚 CD 起算。
- 消耗独立资源**精力**（与弹药分线）。
- 非所有动作可取消：打标签 `Cancellable`（轻击、普通射击）vs `Rooted`（蓄力重击、引导法术，蓄力进度归零；站桩技能完全不可取消）。

### 5.5 怪物意图循环

```
意图评估（决策冷却归零）→ 生成 PendingDraft（攻击/闪避位移/格挡）→ 锁定 → 执行 → 后摇 → 再评估
```

- 怪物锁定后**绝不撤销**；闪避位移、格挡是规划阶段做出的**独立决策**，不是取消。
- 闪避位移无无敌帧，只是走位；玩家 AOE 或预判落点仍可命中。
- 决策冷却：普通怪在动作锁定到后摇结束前不生成新意图；精英/Boss 冷却更短，可触发「预判性追击」惩罚无脑翻滚（消耗 Boss 资源）。
- 高级 AI（推荐）：参考玩家最近 N 次行动模式调整下次出招（延迟攻击抓翻滚后摇），配合「洞察力」形成信息博弈。

## 6. 时间线特权编码

- 玩家：Planning 无限撤销（覆盖草案，零消耗）+ Resolving 翻滚取消（消耗精力）。
- 怪物：锁定不可撤销，保证时间线的「重量感」。
- 系统层守卫：`CancelPrivilege` 组件仅挂玩家；`try_cancel` 无特权一律 `PrivilegeDenied`。

## 7. UI 信息需求（让特权可见）

| 元素 | 作用 |
| :--- | :--- |
| 行动草案指示器 | 显示「待执行：X → 目标」，可点击撤回 |
| 前摇进度条 | 判定帧处有醒目刻度线；进入判定帧区间变红 |
| 怪物意图图标 / 命中帧指示器 | 让玩家判断「现在翻滚能否躲开」 |
| 后摇窗口进度条 | 显示破绽期，提示进攻时机 |
| 精力条 | 与弹药分开展示，提示剩余翻滚次数 |

## 8. 边界情况

- **同时命中（同 hit_clock）**：按 distance → poise → actor_id 破平；互杀结果由破势定先后，需数值验收（见未决问题）。
- **目标死亡**：后续指向该目标的行动取消。
- **弹道飞行中目标移动**：命中帧校验基于当时坐标；`PendingHit` 实体全程可被反制/打断修改。
- **翻滚取消 vs 怪物攻击**：怪物攻击不取消，只是打空 + 额外后摇（奖励玩家）。
- **无敌帧 vs AOE**：AOE 命中帧落在无敌帧内同样无效；落点仍受地面效果（L2 排程）影响。

## 9. 与 Action Graph 的关系

- `ActionTemplate` 提供 phases / hit_frame / cost / cancelable / cooldown / impact 等字段，是 Action Graph 的节点资产。
- `TransitionCondition::InWindow` 即前摇窗口（阶段①）；`ResourceCheck` 对应精力/弹药/冷却门槛。
- 玩家用输入选边（提交翻滚取消），怪物用 Random/策略选边（意图评估）。
- `PendingHit` 是 Action Graph 执行结果的物化实体。

## 10. Bevy 0.19 映射

| 领域概念 | Bevy 0.19 落点 |
| :--- | :--- |
| `CombatPhase` | `#[derive(States)]` + `app.init_state::<CombatPhase>()`；系统组用 `run_if(in_state(...))` |
| 提交/撤销草案、命中广播 | `Message`（`add_message` + `MessageWriter/Reader`，系统间解耦、批量） |
| 命中→受击硬直、反制打断等定向即时响应 | `EntityEvent` + `Observer` + `commands.trigger_targets` |
| 草案、管线、资源 | Component（`PendingDraft`、`ActionPipeline`、`Stamina` 等） |
| 执行队列、全局时间线 | Resource（`ExecutionQueue`、`CombatTimeline`） |
| 输入 | `ButtonInput` 轮询 → 更新草案 / 提交 Message；执行阶段输入实时生效 |
| 时间 | **不使用 `Time`**；逻辑刻度由 Resolving 队列驱动 |
| 测试 | 领域层纯函数单测裁决；App 层 `MinimalPlugins` + `AppPlugin` |

## 11. 未决问题（需要拍板）

1. **规划阶段限时？** 默认无限时（纯策略）；高难度模式可加倒计时。
2. **AI 是否预判玩家取消行为？** 建议方案 B（高级怪参考玩家最近 N 次行动），与「信息即力量」一致。
3. **翻滚取消精力消耗**：固定值 vs 连续递增（建议递增，惩罚无脑滚）。
4. **同时命中互杀**：是否允许同帧互相击杀，还是按破势先定胜负。
5. **DecisionPause 冻结模式**：默认实时推进；是否保留「威胁提示暂停」作为可选项。

## 12. 落地顺序

1. 领域层：`ExecutionQueue` 构建 + 三层裁决（纯函数 + 单测，零 Bevy）。
2. 状态机：Planning ↔ Resolving 切换。
3. 翻滚取消闭环：前摇窗口 + 精力扣除 + 无敌帧。
4. `PendingHit` 实体化：弹道/延迟命中 + 命中帧校验。
5. UI 信息展示（草案、前摇进度条、意图图标、精力条）。
