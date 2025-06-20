// Polyfills for browser environment
import { Buffer } from 'buffer';

// Make Buffer globally available
globalThis.Buffer = Buffer;

// Provide minimal process shim for crypto libraries
if (typeof globalThis.process === 'undefined') {
  globalThis.process = {
    env: {},
    nextTick: (fn, ...args) => Promise.resolve().then(() => fn(...args)),
    version: '',
    versions: { node: '' },
    platform: 'browser',
    browser: true
  };
}

// Export so it can be imported
export { Buffer }; 