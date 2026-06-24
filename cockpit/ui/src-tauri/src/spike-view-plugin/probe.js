// THROWAWAY P4 SPIKE — the gate c-CORS probe: a 2nd same-scheme ES module. If an
// opaque-origin (`sandbox="allow-scripts"`, no same-origin) module graph can `import()`
// this, cross-origin self-code-load works and no inline-bundle fallback is needed.
export const PROBE = 'probe-ok';
