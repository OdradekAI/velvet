import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { isTauri } from '@tauri-apps/api/core';
import { DemoApp } from './demo-app.js';
import { NativeWorkbenchApp } from './native-workbench-app.js';
import '../ui/tokens.css';
import { applyUiThemeSnapshot } from '../ui/theme.js';
import { DEFAULT_UI_THEME } from '../../workbench/plugins/ui-kit.js';
import { VisualQualificationFixture } from './visual-qualification-fixture.js';

const root = document.querySelector('#root');

if (!root) {
  throw new Error('Velvet renderer root was not found.');
}

applyUiThemeSnapshot(DEFAULT_UI_THEME);

const isVisualQualificationFixture = new URLSearchParams(window.location.search).get('fixture') === 'eagle';

createRoot(root).render(
  <StrictMode>
    {isVisualQualificationFixture ? <VisualQualificationFixture /> : isTauri() ? <NativeWorkbenchApp /> : <DemoApp />}
  </StrictMode>,
);
