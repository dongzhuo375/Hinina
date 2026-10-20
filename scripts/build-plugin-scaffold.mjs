// Reuse the locked esbuild dependency owned by Vite; do not download a second toolchain.
import { createRequire } from 'node:module'
const { build } = createRequire(import.meta.resolve('vite'))('esbuild')

await build({
  entryPoints: ['doc/modules/src-tauri/plugin/sdk/scaffold/index.ts'],
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  target: 'es2022',
  outfile: 'src-tauri/target/plugin-scaffold.js',
})
