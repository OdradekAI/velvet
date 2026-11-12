import path from 'node:path';

import type { Config } from '@docusaurus/types';
import referencePages from './reference-pages.json';

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
            '@velvet-ui$': path.resolve(siteDirectory, '../desktop/src/app/ui/index.ts'),
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
  onBrokenAnchors: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },
  future: {
    v4: true,
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
        include: referencePages,
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
          label: 'Plugin development',
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
          type: 'docSidebar',
          sidebarId: 'ui',
          label: 'UI system',
          position: 'left',
        },
        {
          type: 'docSidebar',
          sidebarId: 'maintainers',
          label: 'Maintainers',
          position: 'left',
        },
        { to: '/search', label: 'Search', position: 'right' },
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
