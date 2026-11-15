import type { LinkedDeviceView, PairInviteView } from '../../engine/types';

/** Pixel 8, linked; Handoff is off on the phone's side. */
export const pixel: LinkedDeviceView = {
  id: 'p8',
  name: 'Pixel 8',
  os: 'android',
  fingerprint: 'C4D2·7781',
  state: 'linked',
  features: [
    { feature: 'clipboard', group: 'quiet', local: 'on', remote: 'on', effective: 'on' },
    { feature: 'ring', group: 'quiet', local: 'on', remote: 'on', effective: 'on' },
    { feature: 'files', group: 'ask', local: 'ask', remote: 'ask', effective: 'ask' },
    { feature: 'handoff', group: 'ask', local: 'ask', remote: 'off', effective: 'off' },
  ],
};

export const pixelInvite: PairInviteView = {
  uri: 'carrova://pair?v=1&t=4-maple-orbit@192.168.1.20:47100&k=EXAMPLE',
  qr_svg:
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 25 25"><path d="M0 0h7v7H0zM18 0h7v7h-7zM0 18h7v7H0z"/></svg>',
  code: {
    code: '4-maple-orbit',
    fallback: null,
    renewed: false,
    expired: false,
    expires_in_secs: 160,
  },
  expires_in_s: 160,
};
