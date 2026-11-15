# page-visibility（页面可见性判据）

> 源文件：`src/utils/page-visibility.ts`

## 职责

「页面是否不可见（窗口最小化/切到后台）」的单一判据，供榜单/公告/题目总览/评测列表轮询作暂停条件，避免各处内联 `document.hidden` 产生漂移。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `isPageHidden` | `() => boolean` | `typeof document !== 'undefined' && document.hidden` |

## 直接依赖

无（纯模块）

## 被依赖

- `stores/rankStore.ts`（轮询暂停判据 + 公告可见性补拉同款）
- `stores/announcementStore.ts`（轮询暂停判据与可见性补拉去重）
- `views/SubmissionsView.vue` / `views/ProblemSetView.vue`（列表轮询 isPaused）

## 逻辑流程

无 `document` 的环境（jsdom 部分用例/SSR）按可见处理——不因环境缺失而永久暂停轮询。

## 测试

经消费方用例间接覆盖（各 store/视图 spec 的暂停分支）。
