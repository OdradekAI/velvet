import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { isTauri } from '@tauri-apps/api/core';
import { DemoApp } from './demo-app.js';
import { NativeWorkbenchApp } from './native-workbench-app.js';
import '../ui/tokens.css';

const root = document.querySelector('#root');

if (!root) {
  throw new Error('Velvet renderer root was not found.');
}

createRoot(root).render(
  <StrictMode>
    {isTauri() ? <NativeWorkbenchApp /> : <DemoApp />}
  </StrictMode>,
);
