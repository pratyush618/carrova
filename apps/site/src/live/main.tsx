// The live demos: the desktop app's own screens and components, in their own
// page so the app's global styles never touch the site.
import '@fontsource-variable/atkinson-hyperlegible-next';
import '@fontsource-variable/atkinson-hyperlegible-mono';
import 'virtual:carrova-tokens.css';
import '../../../desktop/src/theme/base.css';
import '../../../desktop/src/i18n/i18n';
import './live.css';
import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { isLiveMessage, type Scene } from './protocol';
import { PairScene } from './PairScene';
import { LinkedScene } from './LinkedScene';
import { NearbyScene } from './NearbyScene';
import { PhoneScene } from './PhoneScene';
import { ReceiveScene } from './ReceiveScene';

const params = new URLSearchParams(window.location.search);
const scene = (params.get('scene') ?? 'receive') as Scene;
const initial = params.get('scheme');
if (initial === 'light' || initial === 'dark') document.documentElement.dataset['theme'] = initial;

/** Bumps on every "play" from the page, so a scene can start over. */
function usePlays(): number {
  const [plays, setPlays] = useState(0);
  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (event.origin !== window.location.origin || !isLiveMessage(event.data)) return;
      if ('scheme' in event.data) document.documentElement.dataset['theme'] = event.data.scheme;
      else setPlays((n) => n + 1);
    };
    window.addEventListener('message', onMessage);
    return () => {
      window.removeEventListener('message', onMessage);
    };
  }, []);
  return plays;
}

function Live() {
  const plays = usePlays();
  if (scene === 'pair') return <PairScene plays={plays} />;
  if (scene === 'phone') return <PhoneScene />;
  if (scene === 'nearby') return <NearbyScene />;
  if (scene === 'linked') return <LinkedScene plays={plays} />;
  return <ReceiveScene />;
}

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <Live />
    </StrictMode>,
  );
}
