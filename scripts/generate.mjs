// Regenerates the checked-in outputs of the tool: the Rust modules of the examples and the
// conformance fixtures recorded with ANTLR's own interpreters.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const rootDir = path.resolve(import.meta.dirname, '..');
const toolJar = path.join(rootDir, 'tool', 'target', 'antlr5-0.0.1-SNAPSHOT-complete.jar');
if (!fs.existsSync(toolJar)) {
  throw new Error('Build the tool first: mvn -B -DskipTests -pl tool -am install');
}

const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'ultra-parser-'));
execFileSync(
  'java',
  ['-jar', toolJar, '-Dlanguage=Rust', '-Xexact-output-dir', '-o', outDir, 'examples/arithmetic/grammar/Arithmetic.g4'],
  { cwd: rootDir, stdio: 'inherit' }
);
for (const file of fs.readdirSync(outDir).filter((file) => file.endsWith('.rs'))) {
  fs.copyFileSync(path.join(outDir, file), path.join(rootDir, 'examples', 'arithmetic', 'src', 'generated', file));
}
fs.rmSync(outDir, { force: true, recursive: true });

execFileSync('java', ['-cp', toolJar, 'GenerateFixtures.java', 'grammars', 'fixtures'], {
  cwd: path.join(rootDir, 'conformance'),
  stdio: 'inherit',
});
