import { isTauri } from '@tauri-apps/api/core';

/**
 * A ref that fits this window's height to `content` as it renders (fonts
 * scale, text wraps, the view changes), keeping its width. The window must
 * be resizable: GTK ignores resize requests on a fixed-size one.
 */
export function fitWindow(content: HTMLElement | null) {
  if (content === null || !isTauri()) return;
  const observer = new ResizeObserver(() => {
    const height = Math.ceil(content.getBoundingClientRect().height);
    // Nothing shown (yet): keep the size rather than collapse the window.
    if (height === 0) return;
    void import('@tauri-apps/api/window')
      .then(({ getCurrentWindow, LogicalSize }) =>
        getCurrentWindow().setSize(new LogicalSize(window.innerWidth, height)),
      )
      .catch(() => undefined);
  });
  observer.observe(content);
  return () => {
    observer.disconnect();
  };
}
