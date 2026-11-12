import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'index',
    {
      type: 'category', label: 'Get started', collapsed: false,
      items: ['platform/overview', 'platform/quick-start', 'platform/choose-contribution', 'platform/capabilities'],
    },
    {
      type: 'category', label: 'Concepts',
      items: ['platform/concepts', 'platform/contributions', 'platform/security'],
    },
    {
      type: 'category', label: 'Build with capabilities',
      items: ['guides/commands-and-views', 'guides/asset-analysis', 'guides/isolated-ui'],
    },
    'platform/ui-bridge', 'platform/integrations', 'platform/testing', 'platform/migration',
    { type: 'link', label: 'API and protocol index', href: '/reference/plugin-api-index' },
    'search',
  ],
  ui: [
    'ui-kit/index', 'ui-kit/showcase',
    {
      type: 'category', label: 'Primitives',
      items: [
        'ui-kit/components/button', 'ui-kit/components/input', 'ui-kit/components/select',
        'ui-kit/components/tabs', 'ui-kit/components/checkbox', 'ui-kit/components/badge',
        'ui-kit/components/icon-button', 'ui-kit/components/progress', 'ui-kit/components/state',
        'ui-kit/components/icon', 'ui-kit/components/text', 'ui-kit/components/stack',
        'ui-kit/components/inline', 'ui-kit/components/separator',
      ],
    },
    {
      type: 'category', label: 'Host overlays',
      items: ['ui-kit/components/dialog', 'ui-kit/components/context-menu', 'ui-kit/components/toast'],
    },
    'ui-kit/customization', 'ui-kit/accessibility',
    { type: 'link', label: 'Host component contracts', href: '/reference/ui-kit-component-guide' },
    { type: 'link', label: 'Third-party UI Bridge', href: '/platform/ui-bridge' },
  ],
  maintainers: [
    'maintenance/index', 'maintenance/documentation-structure',
    { type: 'link', label: 'Registry implementation', href: '/reference/plugin-registry-v1' },
    { type: 'link', label: 'Contract validation matrix', href: '/reference/plugin-contract-matrix' },
    { type: 'link', label: 'Ecosystem qualification', href: '/reference/plugin-ecosystem-qualification' },
    { type: 'link', label: 'UI action coverage', href: '/reference/ui-kit/action-coverage' },
    { type: 'link', label: 'Architecture decisions', href: 'https://github.com/OdradekAI/velvet/tree/main/docs/adr' },
  ],
};

export default sidebars;
