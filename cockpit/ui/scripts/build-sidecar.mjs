// Build the fleetd `serve` binary and place it where Tauri expects the sidecar
// (src-tauri/binaries/fleetd-serve-<target-triple>[.exe]). The binary is
// gitignored, so run this once before `npm run tauri dev|build`.
//
// Profile selection (see Lane P open-question decision in the handoff):
//   - DEBUG by default — keeps the `npm run desktop` dev inner loop fast.
//   - RELEASE when `--release` is passed or `CC_SIDECAR_RELEASE` is truthy.
//     `tauri build` wires this through via the `beforeBuildCommand` so shipped
//     bundles always carry an optimized release sidecar, never a debug binary.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';

const release =
  process.argv.includes('--release') || !!process.env.CC_SIDECAR_RELEASE;
const profileDir = release ? 'release' : 'debug';

// Static commands, arg arrays, no shell — nothing interpolated.
const triple = execFileSync('rustc', ['-Vv']).toString().match(/host:\s*(\S+)/)[1];
const ext = process.platform === 'win32' ? '.exe' : '';

const cargoArgs = ['build', '-p', 'fleetd', '--bin', 'serve'];
if (release) cargoArgs.push('--release');

console.log(`building fleetd serve (${profileDir})…`);
execFileSync('cargo', cargoArgs, {
  cwd: '../..',
  stdio: 'inherit',
});

mkdirSync('src-tauri/binaries', { recursive: true });
const dest = `src-tauri/binaries/fleetd-serve-${triple}${ext}`;
copyFileSync(`../../target/${profileDir}/serve${ext}`, dest);
console.log(`sidecar ready: ${dest}`);
