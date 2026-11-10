// Conventional Commits for every commit and PR title (CI + local hook).
// One config for both, so a squash-merged PR title obeys the same rules.
// Scopes = the workspace's crates/apps plus repo-wide areas; scope is optional.

/** @type {import('@commitlint/types').UserConfig} */
export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    // Keep subjects short (house rule: ~60 chars); 72 leaves room for a scope.
    'header-max-length': [2, 'always', 72],
    'scope-enum': [
      2,
      'always',
      [
        // crates/
        'core', 'recipe', 'manifest', 'transport', 'crypto', 'archive',
        'platform', 'plugin', 'view', 'bundle', 'report',
        // apps/ + tooling
        'cli', 'desktop', 'tauri', 'xtask',
        // repo-wide areas
        'recipes', 'e2e', 'ci', 'deps', 'docs', 'release', 'repo',
      ],
    ],
  },
};
