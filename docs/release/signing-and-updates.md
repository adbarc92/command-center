# Bundle signing & auto-updates (cockpit / Tauri)

How the Fleet Command cockpit is signed and how its auto-updater is wired, plus
**the exact list of CI secrets** the release workflow consumes. Certificate and
key *procurement* is a human/external task (Apple Developer Program, a Windows
Authenticode cert) — this document only describes the wiring and what to obtain.

> **Contract for Lane C (CI):** the secret names in
> [§4 Secrets reference](#4-secrets-reference) are canonical. Reference them as
> `${{ secrets.<NAME> }}` in the release workflow; do not invent new signing
> config in the workflow that belongs in `tauri.conf.json`.

The relevant config lives in
[`cockpit/ui/src-tauri/tauri.conf.json`](../../cockpit/ui/src-tauri/tauri.conf.json):
the `bundle.targets`, `bundle.windows`, `bundle.macOS`, and `plugins.updater`
blocks. All signing identities and the updater public key are **placeholders**
(`null` / `REPLACE_WITH_…`) so that an unsigned local `npm run tauri build`
keeps working with no certs present.

---

## 1. Bundle targets

`bundle.targets` is `["msi", "nsis", "app", "dmg", "appimage", "deb"]`:

| OS      | Targets         | Notes                                              |
| ------- | --------------- | -------------------------------------------------- |
| Windows | `msi`, `nsis`   | MSI (WiX) + NSIS installer. Authenticode-signed.   |
| macOS   | `app`, `dmg`    | `.app` bundle + `.dmg`. Developer ID + notarized.  |
| Linux   | `appimage`, `deb` | AppImage + Debian package. Not code-signed (the updater signature still applies — see §3). |

Each target is only producible on its native OS runner, so the release workflow
fans out across `windows-latest`, `macos-latest`, and `ubuntu-latest`.

`bundle.createUpdaterArtifacts` is `true`, so every build also emits the
`.sig` + updater archive consumed by the auto-updater (§3).

---

## 2. Code signing

Code signing proves the binary came from us and lets the OS run it without
scary warnings (Gatekeeper / SmartScreen). It is **separate** from the updater
signature in §3 — both are required for a production release.

### 2.1 macOS — Developer ID + notarization

Requires an **Apple Developer Program** membership ($99/yr).

What to obtain:

1. A **Developer ID Application** certificate (Apple Developer portal →
   Certificates → "Developer ID Application"). Export it from Keychain as a
   password-protected `.p12`.
2. Your **Team ID** (Apple Developer → Membership).
3. An **app-specific password** for notarization (appleid.apple.com → Sign-In
   and Security → App-Specific Passwords), or an App Store Connect API key.

Wiring:

- `tauri.conf.json` → `bundle.macOS.signingIdentity` is `null` (unsigned dev
  build). In CI, Tauri reads the identity from the imported certificate; the
  certificate itself is provided via env (§4), not hard-coded here.
- Notarization is driven entirely by the `APPLE_*` env vars at build time;
  Tauri staples the ticket automatically.

### 2.2 Windows — Authenticode

Requires a **code-signing certificate** from a CA (DigiCert, Sectigo, etc.).
An EV certificate gives the best SmartScreen reputation but a standard OV cert
also works.

What to obtain:

1. The certificate as a password-protected `.pfx` / `.p12` (or, for an HSM/EV
   token, a thumbprint of the installed cert).

Wiring:

- `tauri.conf.json` → `bundle.windows.certificateThumbprint` is `null`
  (unsigned dev build), with `digestAlgorithm: "sha256"` and a DigiCert
  `timestampUrl` so signatures remain valid after the cert expires.
- In CI, the `WINDOWS_CERTIFICATE` `.pfx` is imported and Tauri signs `msi` +
  `nsis` with it. Provided via env (§4).

---

## 3. Auto-updater

Configured under `plugins.updater` in `tauri.conf.json`:

```jsonc
"plugins": {
  "updater": {
    "pubkey": "",
    "endpoints": [
      "https://releases.command-center.example/cockpit/{{target}}/{{arch}}/{{current_version}}"
    ]
  }
}
```

The updater uses its **own** signing keypair, independent of OS code signing:

- **Public key** (`pubkey`) ships inside the app and verifies update archives.
  It is committed **empty** (`""`) on purpose: a non-empty pubkey makes Tauri
  attempt to *sign* updater artifacts at build time, which fails an unsigned
  local build that has no private key. Generate the keypair with
  `npm run tauri signer generate -- -w ~/.tauri/cc-updater.key`; the printed
  **public** key is what goes here. (Committing the public key is fine — it is
  public by design.)
- **Private key** signs the updater artifacts at build time and is provided via
  `TAURI_SIGNING_PRIVATE_KEY` (+ password). **Never commit it.**

For **release builds**, CI must (a) inject the real public key — either by
patching `pubkey` or passing `--config` to `tauri build` — and (b) opt into
updater artifacts with `tauri build --` flags / `createUpdaterArtifacts`, so the
`.sig` + update archive are produced and signed with `TAURI_SIGNING_PRIVATE_KEY`.
Local unsigned builds leave `pubkey` empty and skip all of this.

`endpoints` is a placeholder host — replace `releases.command-center.example`
with the real update server / static bucket that serves the `latest.json`
manifest per `{{target}}`/`{{arch}}`/`{{current_version}}`.

> **Activation note:** the `plugins.updater` config block is present, but to
> actually *check for and install* updates at runtime the app must also depend
> on and register `tauri-plugin-updater` (Rust + JS). That wiring is **not yet
> added** (it pulls a new crate + a JS dependency) — see the contract request in
> the Lane B report. Until then this block is configuration-only and is ignored
> at runtime; it does not affect unsigned local builds.

---

## 4. Secrets reference

These are the **canonical CI secret names**. Set them in the repo's Actions
secrets; the release workflow (Lane C) references them as
`${{ secrets.<NAME> }}`.

### macOS (Developer ID + notarization)

| Secret                       | What it is                                                    | Where to get it |
| ---------------------------- | ------------------------------------------------------------ | --------------- |
| `APPLE_CERTIFICATE`          | Developer ID Application cert, base64-encoded `.p12`          | Apple Developer → Certificates; `base64 -i cert.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting that `.p12`                              | Set when exporting from Keychain |
| `APPLE_ID`                   | Apple ID email used for notarization                        | Your developer account |
| `APPLE_PASSWORD`             | App-specific password for notarization                      | appleid.apple.com → App-Specific Passwords |
| `APPLE_TEAM_ID`              | 10-char Apple Developer Team ID                             | Apple Developer → Membership |

### Windows (Authenticode)

| Secret                          | What it is                                   | Where to get it |
| ------------------------------- | -------------------------------------------- | --------------- |
| `WINDOWS_CERTIFICATE`           | Code-signing cert, base64-encoded `.pfx`     | CA (DigiCert/Sectigo/…); `base64 -i cert.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD`  | Password protecting that `.pfx`              | Set when exporting / issued by CA |

### Updater (Tauri signing keypair — all platforms)

| Secret                              | What it is                                              | Where to get it |
| ----------------------------------- | ------------------------------------------------------ | --------------- |
| `TAURI_SIGNING_PRIVATE_KEY`         | Private half of the updater keypair (file contents or base64) | `npm run tauri signer generate` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`| Password protecting that private key                  | Chosen at `signer generate` time |

> The matching **public** key is not a secret — it lives in
> `tauri.conf.json` → `plugins.updater.pubkey`.

---

## 5. Local unsigned builds

No secrets and no certs are needed for local development:

```bash
cd cockpit/ui
npm run sidecar        # build the bundled fleetd-serve sidecar (required first)
npm run tauri build    # produces UNSIGNED dev bundles
```

Because every signing identity in `tauri.conf.json` is `null`/placeholder,
the build skips signing and notarization and produces installable (if
OS-untrusted) artifacts. Signing only kicks in when the §4 secrets are present
(i.e. in CI).

---

## 6. Verifying the sidecar supervisor (manual)

The host owns a Rust-side supervisor for the bundled `fleetd-serve` sidecar
(`cockpit/ui/src-tauri/src/sidecar.rs`). To confirm its three guarantees,
run the dev app (`cd cockpit/ui && npm run desktop`) and:

- **Health-gate on launch:** on start the log shows `fleetd: sidecar spawned;
  health-gating on http://127.0.0.1:8787/health` then `fleetd: health OK`. The
  cockpit can reach fleetd (header daemon badge goes live). A `fleetd://status`
  event (`starting` → `ready`) is emitted for the UI.
- **Restart on crash:** kill the child out from under the app —
  `taskkill /F /IM fleetd-serve.exe` (Windows) or `pkill fleetd-serve`. The log
  shows `fleetd: sidecar exited … restarting`, then a fresh spawn + health gate
  within a couple of seconds. The cockpit recovers without an app restart.
- **Kill on close (no orphans):** close the app window, then check
  `tasklist | findstr fleetd-serve` (Windows) / `pgrep fleetd-serve` (unix) —
  it must be **empty**. The supervisor's `shutdown()` runs on `ExitRequested`
  before the app exits, killing the child and preventing a respawn.
