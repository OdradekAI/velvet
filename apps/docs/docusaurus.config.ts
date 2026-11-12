import { stat } from 'node:fs/promises';
import path from 'node:path';

import type { Config, VcsConfig } from '@docusaurus/types';

const siteDirectory = __dirname;
const deployTarget = process.env.DOCS_DEPLOY_TARGET ?? 'local';
const siteUrl = process.env.DOCS_SITE_URL
  ?? (deployTarget === 'github-pages' ? 'https://odradekai.github.io' : 'https://velvet.local');
const siteBaseUrl = process.env.DOCS_BASE_URL
  ?? (deployTarget === 'github-pages' ? '/velvet/' : '/');
// Docusaurus reloads the config for each locale and sets this environment
// variable before loading it. The English build uses the canonical engineering
// path directly because Docusaurus's MDX metadata loader does not preserve
// DocItem metadata through a symlinked content path. The tracked reference/en
// symlink remains available for repository navigation and editor comparison.
const currentLocale = process.env.DOCUSAURUS_CURRENT_LOCALE ?? 'en';
const contentLocaleDirectory = currentLocale === 'zh-Hans' ? 'zh-Hans' : 'en';
const docsContentPath = path.resolve(siteDirectory, 'docs', contentLocaleDirectory);
const referenceContentPath = currentLocale === 'zh-Hans'
  ? path.resolve(siteDirectory, 'reference/zh-Hans')
  : path.resolve(siteDirectory, '../../docs/engineering');
const referenceInclude = [
  'ui-kit/**/*.md',
  'ui-kit-contract-v1.md',
  'ui-kit-component-guide.md',
  'plugin-developer-guide.md',
  'plugin-manifest-v1.md',
  'plugin-registry-v1.md',
  'plugin-contract-matrix.md',
  'plugin-ecosystem-qualification.md',
];

async function readFileMtime(filePath: string) {
  try {
    const fileStats = await stat(filePath);
    return { timestamp: fileStats.mtimeMs, author: '' };
  } catch {
    return null;
  }
}

const filesystemVcs: VcsConfig = {
  initialize: () => {},
  getFileCreationInfo: readFileMtime,
  getFileLastUpdateInfo: readFileMtime,
};

const desktopUiAliasPlugin = function () {
  return {
    name: 'velvet-docs-desktop-ui-alias',
    configureWebpack() {
      return {
        resolve: {
          extensionAlias: {
            '.js': ['.ts', '.tsx', '.js'],
          },
          alias: {
            '@velvet-desktop': path.resolve(siteDirectory, '../desktop/src'),
          },
        },
      };
    },
  };
};

const config: Config = {
  title: 'Velvet Docs',
  tagline: 'Workbench UI Kit and plugin platform documentation',
  favicon: 'img/favicon.svg',
  url: siteUrl,
  baseUrl: siteBaseUrl,
  organizationName: 'OdradekAI',
  projectName: 'velvet',
  onBrokenLinks: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },
  future: {
    v4: true,
    experimental_vcs: filesystemVcs,
    faster: {
      rspackPersistentCache: false,
    },
  },
  i18n: {
    defaultLocale: 'en',
    locales: ['zh-Hans', 'en'],
    localeConfigs: {
      'zh-Hans': { label: '简体中文', htmlLang: 'zh-CN' },
      en: { label: 'English', htmlLang: 'en' },
    },
  },
  themes: ['@docusaurus/theme-classic'],
  plugins: [
    desktopUiAliasPlugin,
    [
      '@docusaurus/plugin-content-docs',
      {
        path: docsContentPath,
        routeBasePath: '/',
        sidebarPath: path.resolve(siteDirectory, 'sidebars.ts'),
        showLastUpdateTime: true,
      },
    ],
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'reference',
        path: referenceContentPath,
        routeBasePath: 'reference',
        sidebarPath: path.resolve(siteDirectory, 'sidebars-reference.ts'),
        include: referenceInclude,
        showLastUpdateTime: true,
      },
    ],
  ],
  themeConfig: {
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: false,
      respectPrefersColorScheme: false,
    },
    navbar: {
      title: 'Velvet Docs',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docs',
          label: 'Plugin Platform',
          position: 'left',
        },
        {
          type: 'docSidebar',
          docsPluginId: 'reference',
          sidebarId: 'reference',
          label: 'API Reference',
          position: 'left',
        },
        {
          href: 'https://github.com/OdradekAI/velvet',
          label: 'GitHub',
          position: 'right',
        },
        {
          type: 'localeDropdown',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentation',
          items: [
            { label: 'Plugin development', to: '/' },
            { label: 'UI Kit reference', to: '/reference/ui-kit-component-guide' },
          ],
        },
        {
          title: 'Project',
          items: [
            { label: 'Architecture', href: 'https://github.com/OdradekAI/velvet/tree/main/docs/adr' },
            { label: 'Changelog', href: 'https://github.com/OdradekAI/velvet/blob/main/CHANGELOG.md' },
          ],
        },
      ],
      copyright: `Velvet ${new Date().getFullYear()}`,
    },
    prism: {
      additionalLanguages: ['bash', 'json', 'rust', 'toml', 'typescript', 'tsx'],
    },
  },
};

export default config;
