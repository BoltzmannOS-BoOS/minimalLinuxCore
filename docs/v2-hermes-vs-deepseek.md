Hermes vs DeepSeek — 对比分析
==========================

══════════════════════
DeepSeek 的实测表现
══════════════════════

10 轮尝试了 6 个不同命令: observe(×3), shell, caps, submit(×2), context-get, debug, commands
覆盖率: 6/26 = 23%

亮点:
  ★ Loop 2: 直接试 shell，探安全底线
  ★ Loop 3: shell 被拒 → caps 查原因 (因果推理)
  ★ Loop 4: 用 submit 尝试绕过 (攻击性思维)
  ★ 报告: 分4个板块, 有表格, 有功能分类

问题:
  ✗ 3 轮浪费在 observe (Loop 1/5/10)
  ✗ 没试 status/daemons/log — 系统读操作都没碰
  ✗ 没试 remember/recall — 核心记忆系统没测试
  ✗ submit 两次失败但不追问原因
  ✗ Loop 9 重复 submit (ALREADY TRIED 被忽略)

══════════════════════
Hermes 评估
══════════════════════

当前 DeepSeek 作为 BoOS 内的自主探索者: 及格 (6/10)

优势:
  - 安全思维 (第一个试 shell)
  - 因果推理 (denied → caps)
  - 报告组织能力

缺陷:
  - 探索不够全面 (23% 覆盖)
  - 不追问失败原因 (submit 失败两次)
  - 浪费轮次在 observe
  - 偶尔无视 ALREADY TRIED

══════════════════════
改进优先级
══════════════════════

1. 精简上下文 — 只发未尝试命令 (当前 26 行全发)
2. 过滤已尝试 — 不发 tried 列表, 直接从可用列表剔除
3. 强化 prompt — 加 "Don't waste turns on observe"
4. 减少 observe — 自动记录, 不占用 DeepSeek 轮次
