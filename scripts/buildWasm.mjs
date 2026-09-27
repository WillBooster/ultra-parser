// Builds the WebAssembly runtime into packages/ultra-parser/ultra_parser.wasm.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const target = 'wasm32-unknown-unknown';
const rootDir = path.resolve(import.meta.dirname, '..');

execFileSync('cargo', ['build', '--release', '--target', target, '--package', 'ultra-parser-wasm'], {
  cwd: rootDir,
  stdio: 'inherit',
});
const metadata = JSON.parse(
  execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], { cwd: rootDir, encoding: 'utf8' })
);
fs.copyFileSync(
  path.join(metadata.target_directory, target, 'release', 'ultra_parser_wasm.wasm'),
  path.join(rootDir, 'packages', 'ultra-parser', 'ultra_parser.wasm')
);
