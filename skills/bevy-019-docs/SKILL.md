---
name: bevy-019-docs
description: 编写、审查或迁移 Bevy 0.19 代码时，查证权威资料（Bevy GitHub、docs.rs/bevy/latest、官方示例与迁移指南、taintedcoders 0.19）并核对 API；包含 0.19 与旧版的关键差异清单。不适用于非 Bevy 的通用 Rust 任务。
---

# Bevy 0.19 资料查询规范

## 何时使用

任何涉及 Bevy API 的编写、review、迁移或文档引用：先查证，再写码。**不要用模型训练记忆或 0.19 之前的教程充当权威**（API 可能已改名或移除）。

## 权威来源（按优先级）

1. docs.rs 搜索：`https://docs.rs/bevy/latest/bevy/?search=<符号>`，读签名与示例。
2. Bevy GitHub examples：`https://github.com/bevyengine/bevy/tree/main/examples`（按类别找官方用法）。
3. 迁移指南：`https://bevy.org/learn/migration-guides/`（尤其 0.18-to-0.19）。
4. 官方发布说明：`https://bevy.org/news/bevy-0-19/`。
5. taintedcoders 0.19 TLDR：`https://taintedcoders.com/bevy/tldr`（非官方，最佳实践补充；另有 input / scenes / ui / picking 等分章）。

## 工作流

1. 定位要用的符号（如 `Message`、`WorldAssetRoot`、`TextureAtlasLayout`）。
2. 在 docs.rs 搜索该符号，读签名与示例；有歧义时对照官方 examples。
3. 涉及 0.19 新机制时，先读 [references/0.19-notes.md](references/0.19-notes.md) 的差异清单。
4. 写完按「禁用清单」自查一遍，确认没有旧 API 写法。

## 关键规则

- 对话记录或旧文档中的示例（`EventReader`、`SceneBundle`、`Camera2dBundle`、`add_state` 等）一律不直接采用，先迁移到 0.19 等价物。
- 引用 API 时在注释或文档标注 Bevy 版本（如 `// bevy 0.19`）。
- 不确定的符号回到 docs.rs / 官方示例验证，禁止猜测。

## 参考资料

- 0.19 关键差异与速查： [references/0.19-notes.md](references/0.19-notes.md)
