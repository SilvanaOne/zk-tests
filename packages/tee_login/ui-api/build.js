import esbuild from 'esbuild';
import { NodeModulesPolyfillPlugin } from '@esbuild-plugins/node-modules-polyfill';

const isDev = process.argv.includes('--dev');
const isWatch = process.argv.includes('--watch');
const isProd = process.argv.includes('--prod');

const config = {
  entryPoints: ['src/api.ts'],
  bundle: true,
  format: 'iife',
  splitting: false,
  outfile: '../ui/public/login-api/v1/api.js',
  platform: 'browser',
  plugins: [NodeModulesPolyfillPlugin()],
  inject: ['src/polyfills.js'],
  define: {
    global: 'globalThis',
    Buffer: 'Buffer',
    ...(isDev && { DEBUG: 'true' }),
    ...(isProd && { DEBUG: 'false' }),
    ...(!isDev && !isProd && { DEBUG: 'false' })
  },
  logOverride: {
    'empty-import-meta': 'silent'
  },
  ...(isProd && { minify: true })
};

if (isWatch) {
  esbuild.context(config).then(ctx => ctx.watch());
} else {
  esbuild.build(config).catch(() => process.exit(1));
} 