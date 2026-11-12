---
id: action-coverage
title: UI Kit action coverage
---

# UI Kit action coverage

UI Kit action coverage 把组件交互映射到 story、browser fixture 和 release gate。

| 组件 | Action | 需要观察的行为 |
| --- | --- | --- |
| Button | `activate` | Enter、Space、busy、disabled 和 callback |
| Input | `change` | value、focus、busy、disabled 和 native input event |
| Select | `change` | option、Arrow key、multiple、busy 和 disabled |
| Tabs | `select`、`navigate` | tablist、roving tabindex、Arrow、Home、End |
| Checkbox | `toggle` | label association、Space、checked 和 disabled |
| Badge | — | child semantics、tone 和 no-action behavior |
| IconButton | `activate` | label、Enter、Space、busy 和 disabled |
| Progress | — | determinate、indeterminate、label 和 value bounds |
| State | `retry` | loading、empty、error、live region 和 retry |

修改键盘模型、state owner、ARIA 语义或 failure behavior 时，应同时更新 component contract、story 和 browser evidence。
