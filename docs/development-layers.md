### Development Layers — the Meta-Automation Upgrade Path

> Like Factorio: Layer 0 is hand-mining, each layer automates the layer below.
> Every upgrade changes HOW we develop, not just WHAT we build.

```
Layer 0: Manual (current terminal workflow)
  你下指令 → 我执行 → 你审查 → 我修正
  = 75 轮攻击/防御，每轮由人触发

Layer 1: Automated Workflow (completed)
  写功能 → 先攻击 → 修复 → 合并
  = 5 条内核增长规则（能力前置于功能、攻击前置于合并、窄接口、默认只读、不信任 agent）
  → 攻击不再是人想起来才做的事，是开发流程的固定步骤

Layer 2: Agent Self-Attack (next)
  agent 记住攻击模式 → 写新功能时自己先攻击 → 自己修 → 人审查
  = SEED.md 的攻击模式库被 agent 自动消费
  → 攻击知识的承载者从人变成系统

Layer 3: Attack Pattern Auto-Evolution
  系统识别新攻击类 → 自动更新防御模板 → agent 用新模板攻击自己
  → 攻击知识自己生长，不再依赖人发现
  → 路径依赖不可避免，但复杂度上限持续升高
```

This is the Seed core meaning applied to development itself:
not "what we do" but "what level of automation we operate at."
The Seed doesn't tell you what to build — it tells you what layer you're on.
