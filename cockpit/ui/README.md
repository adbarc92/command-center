# Fleet Command — cockpit UI

The SP1 cockpit: a tactical command-HUD over the `fleetd` daemon (Vite + Svelte +
TS), optionally wrapped as a lightweight Tauri desktop app.

## Run as a web app (two processes)

```bash
# from the repo root — start the daemon
cargo run -p fleetd --bin serve          # → http://127.0.0.1:8787

# here — start the UI
npm install
npm run dev                              # → http://localhost:5173
```

Launch a **DEMO** mission to watch a unit move through the lifecycle with live
logs/meters (no secrets). Set `VITE_FLEET_URL` to point at a non-default daemon.

## Run as the desktop app (Tauri)

The Tauri shell auto-launches `fleetd` as a sidecar, so you only run one thing:

```bash
npm run sidecar      # build fleetd + place it as the sidecar (once, or after daemon changes)
npm run tauri dev    # launch the desktop window (daemon starts automatically)
```

- `npm run desktop` does both in one step.
- The sidecar binary (`src-tauri/binaries/fleetd-serve-<triple>`) is gitignored;
  `npm run sidecar` rebuilds it.
- Requires the system WebView (WebView2 on Windows, present on Win11).

## Layout

- `src/lib/types.ts` — the daemon event/command contract
- `src/lib/api.ts` — REST + WebSocket client
- `src/lib/fleet.ts` — folds the event stream into per-unit view state
- `src/App.svelte` — the cockpit (new-mission · fleet grid · detail rail)
- `src-tauri/` — the desktop shell (spawns the `fleetd-serve` sidecar)
