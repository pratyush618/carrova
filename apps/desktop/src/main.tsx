import '@fontsource-variable/atkinson-hyperlegible-next';
import '@fontsource-variable/atkinson-hyperlegible-mono';
import 'virtual:carrova-tokens.css';
import './theme/base.css';
import './i18n/i18n';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { NotePopupRoot, notePopupTarget } from './app/popup/NotePopupRoot';
import { detectHost } from './engine/hosts';
import { createPort } from './ipc';

const container = document.getElementById('root');
if (!container) {
  throw new Error('index.html is missing the #root element');
}

const root = createRoot(container);
// A phone notification's pop-up window loads the same bundle with its target in the query.
const popup = notePopupTarget(window.location.search);
void createPort().then((port) => {
  root.render(
    <StrictMode>
      {popup ? (
        <NotePopupRoot port={port} target={popup} />
      ) : (
        <App port={port} host={detectHost()} />
      )}
    </StrictMode>,
  );
});
