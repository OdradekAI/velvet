import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'index',
    {
      type: 'category',
      label: 'Maintainer guide',
      items: ['maintenance/documentation-structure'],
    },
    {
      type: 'category',
      label: 'Plugin Platform',
      link: { type: 'doc', id: 'platform/overview' },
      collapsed: false,
      items: [
        {
          type: 'category',
          label: 'Get started',
          items: [
            'platform/overview',
            'platform/quick-start',
            'platform/choose-contribution',
          ],
        },
        {
          type: 'category',
          label: 'Contributions',
          items: [
            'platform/contributions',
            { type: 'link', label: 'Manifest reference', href: '/reference/plugin-manifest-v1' },
            { type: 'link', label: 'Contract matrix', href: '/reference/plugin-contract-matrix' },
          ],
        },
        {
          type: 'category',
          label: 'UI surfaces',
          items: [
            {
              type: 'category',
              label: 'UI Kit',
              link: { type: 'doc', id: 'ui-kit/index' },
              items: [
                'ui-kit/index',
                'ui-kit/showcase',
                {
                  type: 'category',
                  label: 'Components',
                  items: [
                    'ui-kit/components/button',
                    'ui-kit/components/input',
                    'ui-kit/components/select',
                    'ui-kit/components/tabs',
                    'ui-kit/components/checkbox',
                    'ui-kit/components/badge',
                    'ui-kit/components/icon-button',
                    'ui-kit/components/progress',
                    'ui-kit/components/state',
                    'ui-kit/components/icon',
                    'ui-kit/components/text',
                    'ui-kit/components/stack',
                    'ui-kit/components/inline',
                    'ui-kit/components/separator',
                    { type: 'link', label: 'All component manuals', href: '/reference/ui-kit/components' },
                  ],
                },
                {
                  type: 'category',
                  label: 'Overlays',
                  items: [
                    'ui-kit/components/dialog',
                    'ui-kit/components/context-menu',
                    'ui-kit/components/toast',
                  ],
                },
                'ui-kit/customization',
                'ui-kit/accessibility',
                { type: 'link', label: 'UI Kit contract', href: '/reference/ui-kit-contract-v1' },
                { type: 'link', label: 'Component guide', href: '/reference/ui-kit-component-guide' },
              ],
            },
            'platform/ui-bridge',
          ],
        },
        'platform/security',
        'platform/testing',
        'platform/integrations',
        'platform/migration',
      ],
    },
  ],
};

export default sidebars;
