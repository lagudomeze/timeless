# 3D 素材与伪 3D 方案

## 模型与地形

- **Hyper3D**：免费地形模型，GLB / OBJ / FBX 可直接导入 Bevy。
- **TerraForge3D**：程序化地形生成。
- **OpenGameArt**：低多边形地形块（逐项看许可）。

## PBR 纹理

- **Poly Haven**：CC0 纹理与 HDR。
- **ShareTextures**：4K PBR 纹理。
- **PolyScan**：高质量 PBR 纹理。

## 角色与动作

- **Mixamo**（Adobe）：免费角色与动作，导出 FBX；经 Blender 转 glTF（.glb）后供 Bevy 加载。
- 转换要点：确认动作可用 Bevy 的 glTF 动画加载（`gltf_animation` feature）；导出时检查贴图是否内嵌，避免路径丢失。

## 伪 3D 方案（3D 场景 + 2D 纸片）

适用：俯视 / 斜视角战棋，角色、道具、特效用 2D 精灵。

- 纸片始终面向摄像机（每帧让 Transform 朝向相机，或自定义 Billboard 系统）；脚下加半透明圆形贴地阴影建立「立体感」。
- 树木、石头用 3D 模型，角色 / 道具 / 特效用纸片；用场景体积雾与全局光提升融合度。
- 取舍：光影画死、纸片无厚度（侧转穿帮）、无体积遮挡；漂浮物绕 Y 轴旋转可脑补「厚度」。
- 贴地阴影用独立 Sprite（固定朝下），不随角色朝向旋转。
