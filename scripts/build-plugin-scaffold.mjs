// Reuse the locked esbuild dependency owned by Vite; do not download a second toolchain.
import { createRequire } from 'node:module'
const [major, minor] = process.versions.node.split('.').map(Number)
if (major < 20 || (major === 20 && minor < 6)) {
  throw new Error('Plugin scaffold build requires Node.js >=20.6 (import.meta.resolve).')
}
const { build } = createRequire(import.meta.resolve('vite'))('esbuild')

// Usage: node build-plugin-scaffold.mjs [entry] [outfile]
// Without arguments it builds the scaffold (original behavior); the balloon
// example passes its own entry/outfile via the plugin:balloon:build script.
const [entry, outfile] = process.argv.length > 2
  ? [process.argv[2], process.argv[3] ?? 'src-tauri/target/plugin.js']
  : ['doc/modules/src-tauri/plugin/sdk/scaffold/index.ts', 'src-tauri/target/plugin-scaffold.js']

await build({
  entryPoints: [entry],
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  target: 'es2022',
  outfile,
})
console.log(`Built ${outfile}; copy it to your plugin directory as index.js (matching manifest.entry).`)
