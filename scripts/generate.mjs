// Regenerates the checked-in outputs of the tool: the parsers of the examples.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const rootDir = path.resolve(import.meta.dirname, '..');
const toolJar = path.join(rootDir, 'tool', 'target', 'antlr5-0.0.1-SNAPSHOT-complete.jar');
if (!fs.existsSync(toolJar)) {
  throw new Error('Build the tool first: mvn -B -DskipTests -pl tool -am install');
}

for (const example of fs.readdirSync(path.join(rootDir, 'examples'))) {
  const grammarDir = path.join(rootDir, 'examples', example, 'grammar');
  // Skip entries that are not examples, such as .DS_Store.
  if (!fs.existsSync(grammarDir)) continue;
  const generatedDir = path.join(rootDir, 'examples', example, 'src', 'generated');
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'ultra-parser-'));
  const grammars = fs.readdirSync(grammarDir).filter((file) => file.endsWith('.g4'));
  execFileSync('java', ['-jar', toolJar, '-visitor', '-Xexact-output-dir', '-o', outDir, ...grammars], {
    cwd: grammarDir,
    stdio: 'inherit',
  });
  fs.rmSync(generatedDir, { force: true, recursive: true });
  fs.mkdirSync(generatedDir, { recursive: true });
  for (const file of fs.readdirSync(outDir).filter((file) => file.endsWith('.ts'))) {
    const source = fs.readFileSync(path.join(outDir, file), 'utf8');
    const directive =
      file.endsWith('Lexer.ts') || file.endsWith('Parser.ts')
        ? '// oxlint-disable unicorn/no-null -- Generated ANTLR vocabulary and contexts use null.\n'
        : '';
    fs.writeFileSync(path.join(generatedDir, file), `${directive}${source}`);
  }
  execFileSync(path.join(rootDir, 'node_modules', '.bin', 'oxfmt'), ['--write', generatedDir], { stdio: 'inherit' });
  execFileSync(
    path.join(rootDir, 'node_modules', '.bin', 'oxlint'),
    ['--fix', '--type-aware', '--type-check', generatedDir],
    {
      stdio: 'inherit',
    }
  );
  fs.rmSync(outDir, { force: true, recursive: true });
}
