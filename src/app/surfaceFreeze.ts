/**
 * C3 product-surface freeze flags.
 *
 * Frozen features remain in the tree for compatibility but must not be the
 * default installed-product path.
 */

/** Live multiplayer / Yjs relay collaboration — frozen for beta. */
export const SURFACE_LIVE_COLLAB = false;

/**
 * Free-form Markdown document editor (comments, suggestions, host Save).
 * Available as an explicit secondary route only — not the default shell.
 */
export const SURFACE_LEGACY_DOCUMENT = true;

/** Welcome-markdown / share-first onboarding — frozen. */
export const SURFACE_WELCOME_MARKDOWN = false;

/** Show collab peer counts and room ids in chrome. */
export const SURFACE_COLLAB_CHROME = false;
