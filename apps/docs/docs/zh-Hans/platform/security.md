---
id: security
title: 安全
sidebar_label: 安全
---

# 安全

UI 位置不会自动获得数据或系统权限。有效 capability 由 manifest、trust policy、Profile/Workspace policy、scope、zone 和宿主 grant 共同决定。

## 保护边界

- 插件不能访问宿主 React component、DOM、Tailwind class 或源文件路径。
- UI Bridge 只接收 bounded context、semantic theme、locale、size 和 lifecycle。
- Message schema、origin、entrypoint scheme、版本和 payload size 都在边界验证。
- Host Overlay、焦点恢复、Job state、Asset identity、Layout 和 Preview fallback 由宿主拥有。
- 崩溃、超时、disable、uninstall 和 incompatible surface 只影响当前 extension。

完整的 Alpha 限制和 qualification gate 见[插件生态 qualification](/reference/plugin-ecosystem-qualification)和[插件 contract matrix](/reference/plugin-contract-matrix)。
