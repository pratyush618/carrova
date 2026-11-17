import { useEffect } from 'react';
// CC0, synthesised for Carrova (packaging/README.md "Bundled sounds").
import ringtone from './ringtone.wav';

/** Loop the ringtone while `on`; it stops when `on` turns false or the caller unmounts. */
export function useRingtone(on: boolean): void {
  useEffect(() => {
    if (!on) return;
    const audio = new Audio(ringtone);
    audio.loop = true;
    // A webview that refuses to play without a gesture still shows the call.
    Promise.resolve()
      .then(() => audio.play())
      .catch(() => undefined);
    return () => {
      audio.pause();
    };
  }, [on]);
}
