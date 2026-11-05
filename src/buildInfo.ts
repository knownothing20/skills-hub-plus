/**
 * Version and build identity, resolved at build time by Vite (`define`), so a
 * running binary always reports exactly which source it was compiled from.
 */

export const APP_VERSION = __APP_VERSION__
export const BUILD_DATE = __BUILD_DATE__
export const GIT_SHA = __GIT_SHA__

/** e.g. `v1.0.0 · 2026-11-05 · 10c364b` */
export const APP_BUILD_LABEL = `v${APP_VERSION} · ${BUILD_DATE} · ${GIT_SHA}`

/** Short form for tight spaces: `v1.0.0 (10c364b)` */
export const APP_VERSION_LABEL = `v${APP_VERSION} (${GIT_SHA})`
