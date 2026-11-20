/** Code-less moves between trusted computers: choosing one, waiting for it, a request arriving here. */
export const request = {
  counts: '{{apps}} · {{size}}',
  countsKeys: '{{apps}} · {{size}} · includes keys',
  to: {
    next: 'Choose a computer',
    title: 'Send your setup to…',
    trusted: 'No code needed',
    linked: 'Linked',
    away: 'Away',
    send: 'Send',
    sendTo: 'Send to {{name}}',
    wake: 'Wake',
    wakeName: 'Wake {{name}}',
    fresh: 'Needs a code once',
    freshSub: 'After this move, no code',
    freshFoot:
      'Sending without a code only asks. Nothing is written there until someone at that computer reviews the plan and approves it; keys still need their own yes.',
    useCode: 'Use a code',
    emptyTitle: 'No computers you trust yet',
    emptyBody:
      'The first move to a computer needs a code. After that the two trust each other, and you can send without one.',
    withCode: 'Send with a code',
  },
  wait: {
    waiting: 'Waiting for {{name}}',
    waitingBody:
      'A request is on its screen: {{counts}}. Nothing is sent until someone there reviews and approves it.',
    waitingBodyPlain:
      'A request is on its screen. Nothing is sent until someone there reviews and approves it.',
    timeLeft: 'Time left to answer',
    left: '{{time}} left · the request expires after 2 minutes',
    reviewing: '{{name}} is reviewing',
    reviewingBody:
      "They're going through the plan and choosing what to accept. The move starts when they approve. You can close this; it carries on.",
    declined: '{{name}} declined',
    declinedBody:
      'Nothing was sent. Whoever is there said no to this move. Maybe check with them, or send less.',
    timeout: 'No answer from {{name}}',
    timeoutBody:
      "Nobody approved it within 2 minutes, so the request expired. Requests don't wait longer, so an old one can't be accepted later. Nothing was sent.",
    away: "Can't reach {{name}}",
    awayBody: 'It went to sleep or left this network before answering. Nothing was sent.',
    asleep: 'Asleep? Wake it, or wait until someone is at it.',
    network: 'Same network? Both need the same Wi-Fi or wired network.',
    needsCode: '{{name}} needs a code now',
    needsCodeBody:
      "It won't take a move from this computer without a code anymore. Nothing was sent.",
    cancelMove: 'Cancel move',
    changeWhat: 'Change what moves',
    sendAgain: 'Send again',
    tryAgain: 'Try again',
    useCode: 'Use a code',
  },
  card: {
    title: '{{name}} wants to send you its setup',
    sub: '{{counts}} · {{time}} left',
    review: 'Review',
    decline: 'Decline',
    busy: 'Finish the current move first',
  },
  expired: {
    title: 'A move request from {{name}} expired',
    body: "Requests last 2 minutes. Nothing changed here. Ask them to send again when you're at this computer.",
    dismiss: 'Dismiss',
  },
  device: {
    group: 'Move your setup',
    what: 'Apps, settings and keys',
    whatSub:
      'No code needed: you trust each other. Every move is reviewed on the computer receiving it.',
    sendSetup: 'Send setup',
    requests: 'Send requests',
    requestsOn:
      '{{name}} can send you a move without a code. You still review it, and keys still ask.',
    requestsOff: 'Off: {{name}} needs a code to send you a move.',
  },
  visibleFoot:
    'Move requests from trusted computers only arrive while this computer is visible to Known devices or Everyone. With No one, a move needs a code.',
} as const;
