// Reuse the locked esbuild dependency owned by Vite; do not download a second toolchain.
import { createRequire } from 'node:module'
const [major, minor] = process.versions.node.split('.').map(Number)
if (major < 20 || (major === 20 && minor < 6)) {
  throw new Error('Plugin scaffold build requires Node.js >=20.6 (import.meta.resolve).')
}
const { build } = createRequire(import.meta.resolve('vite'))('esbuild')

await build({
  entryPoints: ['doc/modules/src-tauri/plugin/sdk/scaffold/index.ts'],
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  target: 'es2022',
  outfile: 'src-tauri/target/plugin-scaffold.js',
})
console.log('Built src-tauri/target/plugin-scaffold.js; copy it to your plugin directory as index.js (matching manifest.entry).')
