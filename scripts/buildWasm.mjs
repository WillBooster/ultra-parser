// Builds the arithmetic example for WebAssembly into examples/arithmetic/dist.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const target = 'wasm32-unknown-unknown';
const rootDir = path.resolve(import.meta.dirname, '..');
const distDir = path.join(rootDir, 'examples', 'arithmetic', 'dist');

execFileSync('cargo', ['build', '--release', '--target', target, '--package', 'ultra-parser-arithmetic'], {
  cwd: rootDir,
  stdio: 'inherit',
});
const metadata = JSON.parse(
  execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], { cwd: rootDir, encoding: 'utf8' })
);

fs.rmSync(distDir, { force: true, recursive: true });
// `--target web` emits glue whose `initSync` takes a compiled `WebAssembly.Module`, which is what
// Cloudflare Workers hand out for `.wasm` imports.
execFileSync(
  'wasm-bindgen',
  [
    '--target',
    'web',
    '--out-dir',
    distDir,
    '--out-name',
    'arithmetic',
    path.join(metadata.target_directory, target, 'release', 'ultra_parser_arithmetic.wasm'),
  ],
  { stdio: 'inherit' }
);
