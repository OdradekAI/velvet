---
id: customization
title: Customization and theming
sidebar_label: Customization
---

# Customization and theming

The public UI customization surface consists of semantic tokens, density, contrast, reduced motion and component variants.

## Token snapshot

The UI Bridge sends a read-only `velvet.ui.theme/1` snapshot. Its allow-list includes selected tokens in the `surface.*`, `border.*`, `text.*`, `accent.*`, `status.*` and `focus.*` families, plus `control.height` and `control.radius`. These family names are descriptive, not wildcards granting arbitrary token names.

Component implementation tokens, Tailwind classes, font files, DOM structure and Radix data attributes remain host-private.

## Customization boundaries

| Surface | Entry point |
| --- | --- |
| Trusted wrapper | Semantic props, variants and typed React events |
| Isolated surface | Theme snapshot, semantic component negotiation and the surface's own renderer |
| Global Workbench | Host renderer applies the theme; a plugin cannot write a snapshot back to the host |

See the [UI Kit contract](/reference/ui-kit-contract-v1) for schema and token filtering. Token-compatible rendering does not grant access to host component implementations.
