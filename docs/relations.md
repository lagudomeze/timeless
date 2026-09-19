# 关系模型：物理附着 与 逻辑关系

> ⚠️ **目标设计**：标 🚧 的部分代码里还没有——现状是**只有** `ChildOf`
> （连行动实体都用它）。本篇的规则是接下来要落地的目标（见 `TODO.md` M22）。
>
> 本篇只回答一件事：**两个实体之间的"关系"该用什么表达。**
> 结论一句话：**要跟着 `Transform` 走的用 `ChildOf`；纯逻辑的用自定义关系。**

## 一、为什么必须分开

Bevy 的 `ChildOf` / `Children` 不只是"谁属于谁"，它是**空间层级**：

| 用了 `ChildOf` 会得到 | 对物理附着 | 对逻辑关系 |
| :--- | :--- | :--- |
| 父级 `Transform` 自动传给子级 | ✅ 想要的 | ❌ 毫无意义（逻辑实体往往连 `Transform` 都没有） |
| 父的 `Children` 里多一个元素 | ✅ | ❌ 「孩子」这个词失去意义，层级查询要额外过滤 |
| 父销毁 → 子跟着销毁（`linked_spawn`） | ✅ | ⚠️ 有时候想要，但**不该靠 `ChildOf` 来拿** |
| 参与事件冒泡 / 层级遍历 | ✅ | ❌ 噪声 |

把技能、Buff、行动这类**没有空间位置**的东西挂成 `ChildOf`，短期能跑，长期会让
「谁是谁的孩子」这句话不再可读：`Children` 里混着纸片、阴影、行动实体三种完全
不同性质的东西。

## 二、自定义关系怎么写

Bevy 的关系是一对组件（`Relationship` + `RelationshipTarget`），可以自己定义，
并且**一样能要 `linked_spawn`**（级联销毁）——所以"逻辑关系也能级联"不需要靠 `ChildOf`：

```rust
/// 行动实体 → 行动者（"这一手是谁的"）
#[derive(Component)]
#[relationship(relationship_target = Actions)]
pub struct ActionOf(Entity);

/// 行动者 → 它名下所有还没落地的行动（由 Bevy 的 hook 自动维护）
#[derive(Component)]
#[relationship_target(relationship = ActionOf, linked_spawn)]
pub struct Actions(Vec<Entity>);
```

- `linked_spawn`：行动者被销毁（阵亡 / 重置）时，名下行动跟着销毁——
  和 `Children` 用的是同一套机制，`S5` 拿到的那条"没有孤儿行动"的结构保证不丢。
- 读法：`action_of.parent()` 取行动者（和 `child_of.parent()` 形状一致）。
- 找"某人的行动"：读行动者身上的 `Actions`，不必全表扫描。

**约定**：关系组件自己写，名字用 `…Of`（源）+ 复数（目标）：`ActionOf`/`Actions`、
`EquippedTo`/`EquippedItems`。

## 三、本项目的三类关系

| 关系 | 源 | 目标 | 用哪个 | 为什么 |
| :--- | :--- | :--- | :--- | :--- |
| 单位 → 纸片 / 阴影 | `UnitSprite` / `UnitShadow` | 单位 | **`ChildOf`** | 纸片与阴影是单位根节点的**视觉子节点**，靠局部坐标表达世界偏移（脚底 + 无旋转 + 无缩放） |
| 行动 → 行动者 | 行动实体 | 单位 | **`ActionOf` / `Actions`** | 行动实体没有 `Transform`，归属是纯逻辑（谁能撤它、它忙住了谁） |
| 物品 → 装备槽位 | 物品 | 槽位 | **`EquippedTo` / `EquippedItems`** | 装备逻辑（类型校验 / 卸下）与"跟着动"是两件事 |

**装备的容器与内容分离**：槽位是 PC 的物理延伸 → 槽位挂 `ChildOf(pc)`；
物品是独立实体（可自由脱离）→ 物品挂 `EquippedTo(slot)`。**卸下时要同时移除两者**，
并把 `GlobalTransform` 转回 `Transform`，否则物品会跳到世界原点。

## 四、用关系的两条纪律

1. **关系组件的写入由 hook 维护，不要手改集合**：想改"谁属于谁"就改**源**那一侧
   （`ActionOf` / `EquippedTo` / `ChildOf`），`Actions` / `EquippedItems` / `Children`
   由 Bevy 自动同步。
2. **类型 / 容量校验用 Observer 监听 `OnInsert`**：例如"这个槽只收剑"，就在
   `OnInsert, EquippedTo` 里检查，不符合当场移除——不要在采购 / UI 侧各写一遍。

## 五、待实测（写进文档但还没验证的）

- `bsn!` 里能不能直接写**自定义**关系组件（`ActionOf({actor})`）。
  `ChildOf({actor})` 实测可以（见 `TODO.md` M21：我先按 `Template` 的约束推断"不行"，
  实测打脸），自定义关系走的是同一套关系机制，**大概率可以，但必须实测**——
  这条经验值得记：静态推断不如跑一遍。
