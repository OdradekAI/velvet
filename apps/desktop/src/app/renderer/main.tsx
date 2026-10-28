import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { DemoApp } from './demo-app.js';
import '../ui/tokens.css';

const root = document.querySelector('#root');

if (!root) {
  throw new Error('Velvet renderer root was not found.');
}

createRoot(root).render(
  <StrictMode>
    <DemoApp />
  </StrictMode>,
);
