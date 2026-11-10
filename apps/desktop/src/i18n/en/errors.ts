/** Titles for failures, one per `BridgeError.code`; the engine's own words go below them. */
export const errors = {
  cancelled: 'Stopped',
  peer_cancelled: 'The other computer stopped the move',
  wrong_passphrase: "That passphrase doesn't open this file",
  bad_code: "That isn't a Carrova code",
  pairing_failed: "That code didn't work",
  cannot_connect: "Couldn't find the other computer",
  invalid_selection: 'The selection needs a decision first',
  blocked: 'Something needs your decision first',
  no_space: 'Not enough space',
  network: "Couldn't reach the other computer",
  rolled_back: 'The move was undone; nothing changed',
  rollback_failed: "The move couldn't be fully undone",
  session: "This move can't continue",
  recipes: "Carrova's app list couldn't be loaded",
  internal: 'Something went wrong',
} as const;
