---
id: accessibility
title: Accessibility
sidebar_label: Accessibility
---

# Accessibility

Keyboard behavior, focus model, selection ownership, ARIA semantics and reduced motion are part of the UI Kit contract. Component documentation should identify:

- native or semantic roles;
- keyboard navigation;
- focus entry and restoration;
- disabled, busy, loading, empty and error states;
- live regions and retry behavior;
- high contrast, density and reduced motion.

Local browser fixtures cover portals, focus traps/restoration, native controls, ARIA labels and host-projection keyboard behavior. Their scope is recorded in [UI action coverage](/reference/ui-kit/action-coverage). A passing fixture does not establish screen-reader or native OS/WebView qualification on every supported platform; see the [ecosystem gate](/reference/plugin-ecosystem-qualification).
