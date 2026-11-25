/** Messages the site page sends to a live demo frame (same origin only). */
export type LiveMessage =
  | { readonly type: 'carrova-live'; readonly scheme: 'light' | 'dark' }
  | { readonly type: 'carrova-live'; readonly play: true };

export type Scene = 'pair' | 'receive' | 'phone' | 'nearby' | 'linked';

/** The size each scene is drawn at; the site scales the frame to fit. */
export const SCENE_SIZE: Record<Scene, readonly [number, number]> = {
  pair: [1100, 720],
  receive: [1100, 720],
  phone: [1100, 720],
  nearby: [1100, 720],
  linked: [1100, 720],
};

export function isLiveMessage(data: unknown): data is LiveMessage {
  return (
    typeof data === 'object' &&
    data !== null &&
    (data as { type?: unknown }).type === 'carrova-live'
  );
}
