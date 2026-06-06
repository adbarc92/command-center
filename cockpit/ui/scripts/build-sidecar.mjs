// Build the fleetd `serve` binary and place it where Tauri expects the sidecar
// (src-tauri/binaries/fleetd-serve-<target-triple>[.exe]). The binary is
// gitignored, so run this once before `npm run tauri dev|build`.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';

// Static commands, arg arrays, no shell — nothing interpolated.
const triple = execFileSync('rustc', ['-Vv']).toString().match(/host:\s*(\S+)/)[1];
const ext = process.platform === 'win32' ? '.exe' : '';

console.log('building fleetd serve…');
execFileSync('cargo', ['build', '-p', 'fleetd', '--bin', 'serve'], {
  cwd: '../..',
  stdio: 'inherit',
});

mkdirSync('src-tauri/binaries', { recursive: true });
const dest = `src-tauri/binaries/fleetd-serve-${triple}${ext}`;
copyFileSync(`../../target/debug/serve${ext}`, dest);
console.log(`sidecar ready: ${dest}`);
