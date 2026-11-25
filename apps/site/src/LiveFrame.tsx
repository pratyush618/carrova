import { useEffect, useRef, useState } from 'react';
import { SCENE_SIZE, type LiveMessage, type Scene } from './live/protocol';
import { useScheme } from './theme';

interface LiveFrameProps {
  readonly scene: Scene;
  readonly title: string;
  /** Bump to start the scene over. */
  readonly play?: number;
  /** Decorative: out of the tab order and hidden from assistive tech. */
  readonly decorative?: boolean;
  /** Start over each time it comes well into view. */
  readonly playOnView?: boolean;
}

/** One of the app's live demos (live.html), scaled to fit its box. */
export function LiveFrame({
  scene,
  title,
  play = 0,
  decorative = false,
  playOnView = false,
}: LiveFrameProps) {
  const box = useRef<HTMLDivElement>(null);
  const frame = useRef<HTMLIFrameElement>(null);
  const [scale, setScale] = useState(0);
  const scheme = useScheme();
  const [width, height] = SCENE_SIZE[scene];
  // The scheme it first loads with; later changes go by message, without a reload.
  const [src] = useState(() => `./live.html?scene=${scene}&scheme=${scheme}`);

  useEffect(() => {
    const el = box.current;
    if (el === null) return undefined;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setScale(entry.contentRect.width / width);
    });
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, [width]);

  const post = (message: LiveMessage) => {
    frame.current?.contentWindow?.postMessage(message, window.location.origin);
  };

  useEffect(() => {
    post({ type: 'carrova-live', scheme });
  }, [scheme]);

  useEffect(() => {
    const el = box.current;
    if (!playOnView || el === null) return undefined;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) post({ type: 'carrova-live', play: true });
      },
      { threshold: 0.6 },
    );
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, [playOnView]);

  useEffect(() => {
    if (play > 0) post({ type: 'carrova-live', play: true });
  }, [play]);

  return (
    <div
      ref={box}
      className="live-frame"
      style={{ aspectRatio: `${String(width)} / ${String(height)}` }}
      aria-hidden={decorative || undefined}
    >
      <iframe
        ref={frame}
        src={src}
        title={title}
        width={width}
        height={height}
        loading="lazy"
        tabIndex={decorative ? -1 : undefined}
        style={{ transform: `scale(${String(scale)})` }}
        onLoad={() => {
          post({ type: 'carrova-live', scheme });
          if (play > 0) post({ type: 'carrova-live', play: true });
        }}
      />
    </div>
  );
}
