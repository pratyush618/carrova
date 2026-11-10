import 'react';

/** Custom properties components set at runtime (via CSSOM, CSP-safe). */
declare module 'react' {
  interface CSSProperties {
    /** Progress fill, 0–1. */
    '--p'?: string;
    /** Tree indentation level. */
    '--depth'?: string;
    /** Seal burst particle vector and spin. */
    '--dx'?: string;
    '--dy'?: string;
    '--rot'?: string;
  }
}
