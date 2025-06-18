# TEE Login UI API

This package contains the iframe-based API for secure cryptographic operations within a sandboxed environment.

## Build Commands

- `npm run build` - Standard build (no debug logging)
- `npm run build:dev` - Development build with debug logging enabled
- `npm run build:prod` - Production build (minified, no debug logging)
- `npm run watch` - Watch mode for development

## Output

All builds output to `../ui/public/login-api/v1/api.js` which is loaded by the iframe.

The script is built as an IIFE (Immediately Invoked Function Expression) to ensure compatibility with sandboxed iframes.

## Security Model

The iframe operates in a sandboxed environment with:

- Only `allow-scripts` permission (no `allow-same-origin`)
- Secure memory zeroing for sensitive data
