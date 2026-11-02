# argv（启动参数匹配原语）

> 源文件：`src-tauri/src/infra/argv.rs`

## 职责

启动参数匹配的**唯一来源**：`--console`（`console::requested`）与 `--settings`
（`launch_args::requested`）曾各写一份相同的 `OsString` 扫描，注释声称「语义必须
一致」却无机制保证（评审十九 #9）。收敛到 `has_flag` 后，新增参数只加一行调用。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `has_flag` | `pub(crate) fn has_flag(args: &[OsString], flag: &str) -> bool` | argv（**不含 argv[0]**）中是否存在与 `flag` **完全相同**的参数。近形（`--settings=x`、`-settings`）不算；未知参数静默忽略；`OsString` 入参与非 Unicode 参数共存不 panic |

## 直接依赖

- `std::ffi::OsString`

## 被依赖

- `infra/console.rs`（`requested` → `--console`）
- `infra/launch_args.rs`（`requested` → `--settings`）
- `infra/single_instance.rs`（第二实例参数含 `--settings` 时提示需完全退出后重启）

## 逻辑流程

`args.iter().any(|arg| arg == flag)` —— `OsString: PartialEq<str>` 保证按 OS 串
比较（非 UTF-8 字节序列也不 panic）。

## 测试

`infra/tests/argv_tests.rs`：精确匹配矩阵（等值/近形/混参/不同 flag 互不影响）
与非 Unicode 参数共存。`console_tests` / `launch_args_tests` 继续按各自参数做端到端
语义锁定。
