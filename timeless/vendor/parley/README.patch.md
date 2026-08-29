# parley 0.9.0 本地补丁说明

## 为什么补丁

Bevy 0.19 的 `bevy_text` 使用 parley 0.9.0 排版。parley 0.9.0 在
`src/analysis/mod.rs` 中硬编码 `WordSegmenter::new_for_non_complex_scripts`，
该构造器**不加载中日韩（CJK）词典**。因此 HUD 文案一旦包含中文，每帧排版都会触发
`icu_segmenter` 内部警告：

```text
ICU4X data error: No segmentation model for complex script: Chinese/Japanese
```

该警告并非 panic（中文仍能按“整段回退”方式渲染），但会以每帧数条的速度刷屏。

## 改了什么

1. `src/analysis/mod.rs` 的 `word_segmenter()`：把
   `WordSegmenter::new_for_non_complex_scripts(...)` 换成
   `WordSegmenter::new_auto(...)`。`new_auto` 会加载已编译进
   `icu_segmenter_data` 的 CJK 词典（`cjdict`）与东南亚 LSTM 模型，
   中文/日文获得真正的词典分词，警告消失。
2. `src/bidi.rs` 的 `mask()`：加 `#[allow(deprecated)]`，消掉
   `BidiClass::to_icu4c_value` 的弃用警告（ICU4X 2.3 弃用后无公开替代）。

## 依赖要求

`WordSegmenter::new_auto` 需要 `icu_segmenter` 同时启用
`compiled_data` 与 `auto` 两个 feature：

- `compiled_data`：parley 原本就声明了；
- `auto`：由 `timeless-app/Cargo.toml` 中的直接依赖
  `icu_segmenter = { version = "2.3.0", features = ["auto"] }` 提供
  （cargo feature 合并对整棵依赖树生效）。

## 如何升级 / 移除补丁

当上游 parley 发布修复版本（改用 `new_auto` 或等效方案）后：

1. 删除 `[patch.crates-io]` 段与 `vendor/parley/` 目录；
2. 更新 `bevy` 依赖即可用回 registry 版 parley；
3. 可同时移除 `timeless-app` 对 `icu_segmenter` 的直接依赖（除非其他地方仍需要）。

补丁基于 crates.io `parley 0.9.0` 原样拷贝，除上述两处外无其他改动。
