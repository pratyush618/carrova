/** "Apps Carrova knows" catalogue and app detail. */
export const apps = {
  title: 'Apps Carrova knows',
  show: 'Show',
  filter: { all: 'All', verified: 'Verified', community: 'Community' },
  search: 'Search apps',
  searchPlaceholder_one: 'Search {{count}} app',
  searchPlaceholder_other: 'Search {{count}} apps',
  updated: 'App list updated {{day}}. Signature checked.',
  unsigned: "App list signature couldn't be checked. Carrova is using the list it shipped with.",
  communityTitle: 'Community apps are checked by tests, not reviewed by the Carrova team',
  communityBody: "They're still data only: they can't run code.",
  bundle: {
    signed: 'Signed recipes',
    unsigned: 'Recipes not signed',
    list: 'Recipe list {{version}} · {{apps}}',
    dev: 'Development folder · {{apps}}',
  },
  grid: 'Apps',
  installed: 'Set up on this computer',
  notInstalled: 'Not found here',
  noMatch: 'No apps match “{{query}}”.',
  recipe: {
    intro:
      'What Carrova can move for {{app}}, and where it lives. Sensitive items always start off.',
    items: 'What moves',
    tier: 'Trust',
    tiers: {
      verified: 'Reviewed by the Carrova team and signed.',
      community: 'Written by the community, checked by tests, signed.',
      unverified: 'Loaded from a development folder, not signed.',
    },
    here: 'On this computer',
  },
} as const;
