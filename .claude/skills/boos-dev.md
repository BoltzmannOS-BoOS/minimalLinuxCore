---
name: boos-dev
description: BoOS/minimalLinuxCore 项目开发规范。在编写、修改、重构此项目代码时必须遵循。
---

# BoOS/minimalLinuxCore 开发规范

## 技术栈约束

- **语言**: POSIX shell (`#!/bin/sh`)，禁止使用 bash 专有扩展（如 `[[`、数组、`source` 等）
- **运行时**: BusyBox 提供的工具链 + Linux kernel
- **虚拟化**: QEMU x86_64，nographic 模式，内核启动参数 `console=ttyS0 rdinit=/init`
- **根文件系统**: initramfs（cpio newc + gzip），无持久化磁盘
- **文件系统布局**: `/run/boos/` 为运行时数据，`/etc/boos/` 为静态配置
- **构建**: shell 脚本调用 `find . | cpio -H newc -o | gzip`

## 项目架构（不可违反）

### 组件职责分离

```
boos-shell    — 人机交互界面，解析用户输入，路由命令
boos-exec     — 命令执行引擎，无交互。注册表查找 → 能力检查 → 执行
boos-submit   — 将请求写入 /run/boos/requests/ 文件
boos-process  — 扫描请求目录，逐个调 boos-exec，写结果文件
boos-daemon   — 后台无限循环，每隔 1s 调 boos-process
/init         — 挂载虚拟文件系统，创建目录，启动 daemon，exec shell
```

### 两条执行路径

```
# 直接路径（人工交互）
用户 → boos-shell → boos-exec → 能力检查 → 执行

# 请求队列路径（AI/程序调用）
提交者 → boos-submit → /run/boos/requests/req-*
       → boos-daemon → boos-process → boos-exec
       → /run/boos/results/req-*.out
```

### 命令注册表模式

每个命令一个 `.cmd` 文件，放在 `/etc/boos/commands/`，四个字段必填：

```ini
name=<命令名>
capability=<对应的allow_xxx键>
description=<一句话描述>
exec=<__builtin_xxx 或 /bin/xxx 或 /path/to/executable>
```

**规则**：
- 简单内建功能（展示、查询）用 `__builtin_` 前缀，在 `boos-exec` 的 `run_builtin_or_exec` 函数中实现
- 需要子进程的功能用 `exec=/bin/xxx` 或外部脚本路径
- 新增命令 = 新增 `.cmd` 文件 + 在 `boos-exec` 中添加对应的 `__builtin_` 分支（如为内建）+ 在 `capabilities.conf` 添加对应能力开关

### 能力系统模式

`/etc/boos/capabilities.conf` 使用 `allow_<name>=0|1` 格式。

读取能力值的标准写法：
```sh
get_cap_value() {
    key="$1"
    [ ! -f "$CAP_FILE" ] && { echo "0"; return; }
    grep "^$key=" "$CAP_FILE" | cut -d '=' -f 2
}
```

## 代码风格规范

### 命名约定

| 类型 | 风格 | 示例 |
|------|------|------|
| 路径/常量 | `UPPER_CASE` | `LOG_FILE`, `REQ_DIR`, `CAP_FILE` |
| 函数名 | `snake_case` | `log_msg`, `get_field`, `require_cap` |
| 文件扩展名 | `.sh` 仅构建脚本，BoOS 内部组件无扩展名 | `boos-exec` 不是 `boos-exec.sh` |
| 命令注册文件 | `.cmd` | `status.cmd`, `help.cmd` |

### 函数标准模板

```sh
# 日志函数（每个脚本都要有）
LOG_FILE="/var/log/boos.log"
log_msg() {
    echo "[组件名] $1" >> "$LOG_FILE"
}

# 字段读取（用于 .cmd / .conf 文件）
get_field() {
    file="$1"
    key="$2"
    grep "^$key=" "$file" | cut -d '=' -f 2-
}
```

### 文件迭代固定写法

```sh
for f in /path/pattern-*
do
    [ -f "$f" ] || continue
    # 处理 $f
done
```

### 错误处理

- 空值检查：`[ -z "$var" ]` 或 `[ -z "$1" ]`
- 文件存在检查：`[ ! -f "$file" ]`
- 失败返回：`return 1`（函数内），不主动 `exit`（保持 shell 不退出）
- 构建脚本用 `set -e`，BoOS 运行时组件不用

### 请求 ID 生成

```sh
id="$(cat /proc/uptime | cut -d ' ' -f 1 | tr -d '.')"
file="$REQ_DIR/req-$id"
```

## 新增组件清单

添加一个新的 BoOS 命令需要触碰以下文件（全部必须）：

1. `rootfs/etc/boos/commands/<name>.cmd` — 注册命令
2. `rootfs/etc/boos/capabilities.conf` — 添加 `allow_<name>=1` 或 `=0`
3. `rootfs/bin/boos-exec` — 在 `run_builtin_or_exec` 中添加 `__builtin_<name>` 分支（如为内建命令）
4. `rootfs/bin/boos-shell` — 在 `case "$cmd" in` 中添加命令路由（如需要特殊参数处理）
5. `README.md` 和 `PLAN.md` — 更新命令列表和进度

## 设计原则

1. **每个操作可归因** — 日志记录谁做了什么
2. **先注册再执行** — 不在注册表中的命令无法执行
3. **分离关注点** — shell 只管界面，exec 只管执行
4. **能力显式声明** — 不默认信任任何命令
5. **文件即接口** — 组件间通过 `/run/boos/` 下的文件通信，不通过函数调用或管道
6. **保持最小化** — 不引入包管理、不写 C 扩展（除非 M12 明确启动）、不添加不必要的 BusyBox applet

## 测试规范

每次改动后验证步骤：
```sh
./scripts/build-rootfs.sh    # 构建
./scripts/run-qemu.sh        # 启动（Ctrl+A, X 退出）
```

在 BoOS shell 内执行：
```
help
commands
status
submit status
results
log
```

如涉及权限改动，额外测试：
```
submit <被禁命令>
results          # 应看到 Permission denied
```

## 禁止事项

- 不要在 BoOS 组件中使用 bash 扩展语法
- 不要在 `boos-shell` 中直接执行命令逻辑（必须交给 `boos-exec`）
- 不要绕过能力检查直接执行
- 不要在构建脚本中使用 `set -e` 之外的错误处理（保持简单）
- 不要创建 `/run/boos/` 之外的运行时目录
- 不要给 shell 脚本加 `.sh` 扩展名（构建脚本除外）
- 不要凭空添加没有对应 `.cmd` 注册文件的命令
