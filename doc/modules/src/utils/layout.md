# layout（布局偏好值域）

> 源文件：`src/utils/layout.ts`

## 职责

布局偏好取值域的唯一权威模块：题面/编辑器分栏比例的默认值、上下界与归一函数。前端所有「分栏比例」相关校验/兜底都从这里取常量（`config.service` 的读取归一、设置页滑杆 min/max、解题页拖拽钳制），不再有第二份字面量。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `DEFAULT_SPLIT_RATIO` | `0.48` | 分栏比例兜底值（设计稿 48% / 52%；与 Rust `default_split_ratio` 一致） |
| `SPLIT_RATIO_MIN` / `SPLIT_RATIO_MAX` | `0.3` / `0.7` | 分栏比例上下界（与 Rust `LayoutConfig::sanitize` 的钳制同域） |
| `clampSplitRatio` | `(raw: unknown) => number` | 分栏比例归一：非数字/非有限值回退 `0.48`，有限越界值钳到边界（镜像 Rust sanitize 语义） |

## 直接依赖

无（纯常量与纯函数模块）。

## 被依赖

- `services/config.service.ts` — `getSplitRatio` 读取归一（读取路径不经过后端 sanitize，手改 `config.json` 的越界值在此兜底）
- `composables/useSettingsForm.ts` — `clampRatio`（本模块钳位 + 滑杆 step 两位小数舍入的包装）、表单初始值
- `components/settings/LayoutSection.vue` — 滑杆 `min`/`max` 绑定
- `views/ProblemSolveView.vue` — 初始值、拖拽/键盘调整钳制、separator 的 aria 值域派生

## 逻辑流程

```
clampSplitRatio(raw)
  非数字 / 非有限值（NaN / Infinity）→ DEFAULT_SPLIT_RATIO
  有限值 → clamp 到 [SPLIT_RATIO_MIN, SPLIT_RATIO_MAX]

值域变更需同步两处：本文件、Rust `LayoutConfig::sanitize`（split_ratio 钳制 +
非有限值回退默认）。前端消费方一律 import 本模块常量，禁止再写字面量
（曾出现三处分头写死：config.service 0.2–0.8 透传带、useSettingsForm 0.3/0.7、
ProblemSolveView 拖拽 0.3/0.7 —— 手改 0.25 会原样进解题页、首次拖拽即跳变）
```

设计要点：

- **读取路径必须归一**：Rust `sanitize` 只在保存路径（`update_config`）执行，读取
  不经钳制 —— 手改 `config.json` 写 0.25 会原样到达前端；若读取侧用宽透传带放行，
  解题页按 0.25 加载后任何一次拖拽都会跳变入 [0.3, 0.7]。
- **钳到边界而非整值回退**：有限越界值钳到边界保留手改者意图的方向（0.25→0.30、
  0.95→0.70），与 Rust `sanitize` 的 `clamp` 一致；只有非数字/非有限值才回退默认。
- **刻意不做两位小数舍入**：解题页拖拽的单次增量可能小于 0.005，通吃舍入会卡死
  增量；与滑杆 step 0.01 对齐的舍入只属于设置页的 `clampRatio` 包装。
