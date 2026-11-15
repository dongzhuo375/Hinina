# SettingsField（设置表单字段脚手架）

> 源文件：`src/components/settings/SettingsField.vue`

## 职责

设置页表单字段四件套（label 文本 + 控件 + 错误行/提示行）的骨架组件：OJ 服务器与编辑器分组的文本/数字/下拉字段共用。控件本身经默认 slot 由调用方提供（v-model、校验态 class、disabled 留在调用点）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `label` | prop `string` | 字段标签文本 |
| `locked` | prop `boolean?` | 锁定态：label 降为 muted 色（维护者钥匙语义）；控件禁用由调用方处理 |
| `error` | prop `string \| null?` | 校验错误，存在时替代提示行 |
| `hint` | prop `string?` | 常态提示文案 |
| default slot | slot | 控件（input/select） |

## 直接依赖

- `vue`（`defineProps`）

## 被依赖

- `components/settings/OjServerSection.vue`（服务器地址/比赛 ID/密码/四个数字字段）
- `components/settings/EditorSection.vue`（字号/Tab 宽度/默认语言/自动保存间隔）
- `components/settings/ThemeSection.vue`（两个只读下拉）

## 逻辑流程

`<label>` 包裹控件的 DOM 形态是测试契约（`SettingsView.behavior.spec` 的 `field()` 按「label 首个直接子 span 文本 → label 内 input/select」定位控件），不得改为平铺。错误与提示互斥（error 优先）；OJ 分组的「当前 OJ」字段错误与安抚提示并存，属独有形态，不走本组件。

## 测试

`src/components/settings/__tests__/SettingsField.spec.ts` 锁定 label 首子 span 契约、error/hint 互斥与 locked 降色。
