import type { ReactNode } from 'react';
import { useDoc } from '@docusaurus/plugin-content-docs/client';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import OriginalContent from '@theme-original/DocItem/Content';
import pageMetadata from '../../../generated/page-metadata.json';

type PageMetadata = {
  readonly audience: string;
  readonly translationStatus: string;
  readonly stale: boolean;
};

const notices = {
  en: {
    extension: 'Product-extension contracts · In-repository Alpha; no published external SDK.',
    'host-ui': 'Trusted host UI · React wrappers and host projections are not third-party SDK imports.',
    fixture: 'Fixture guide · HostFacade methods model host seams; they are not shipped SDK APIs.',
    maintainer: 'Maintainer documentation · Implementation and qualification evidence; not a plugin API promise.',
  },
  'zh-Hans': {
    extension: '产品扩展契约 · 仓库内 Alpha，尚未发布外部 SDK。',
    'host-ui': '可信宿主 UI · React wrapper 和宿主投影不是第三方 SDK 导入接口。',
    fixture: 'Fixture 指南 · HostFacade 方法模拟宿主边界，不是已发布的 SDK API。',
    maintainer: '维护者文档 · 实现与验证证据，不构成插件 API 承诺。',
  },
} as const;

export default function DocItemContent(props: { readonly children: ReactNode }): ReactNode {
  const { metadata } = useDoc();
  const { siteConfig, i18n } = useDocusaurusContext();
  const locale = i18n.currentLocale === 'zh-Hans' ? 'zh-Hans' : 'en';
  const defaultBase = siteConfig.baseUrl.replace(/zh-Hans\/$/, '');
  const route = ('/' + metadata.permalink.slice(defaultBase.length).replace(/^zh-Hans\//, '')).replace(/\/$/, '') || '/';
  const record = (pageMetadata as Record<string, PageMetadata>)[route];
  const audience = record?.audience as keyof typeof notices.en | undefined;
  const englishUrl = defaultBase + route.replace(/^\//, '');
  return (
    <>
      {audience && <aside className="velvet-doc-notice" aria-label={locale === 'en' ? 'Document audience' : '文档适用范围'}>{notices[locale][audience]}</aside>}
      {locale === 'zh-Hans' && record && (record.translationStatus !== 'reviewed' || record.stale) && (
        <aside className="velvet-doc-notice velvet-doc-notice--translation" aria-label="翻译状态">
          {record.stale ? '英文或中文内容已变化，本页尚未重新核对。' : record.translationStatus === 'summary' ? '本页是中文摘要，未覆盖英文原文的全部契约细节。' : '本页译文尚未完成一致性复核。'}
          {' '}<a href={englishUrl} lang="en">查看英文原文</a>
        </aside>
      )}
      <OriginalContent {...props} />
    </>
  );
}
