# Host-provided charts in Phira multiplayer (experimental)

This fork adds **private, room-scoped local charts** to its existing multiplayer flow. It does not change the Phira login requirement.

## Run the matching server

Both the client and the server must use the vendored protocol in `vendor/phira-mp`. **Unmodified TeamFlos/Phira MP servers and unmodified Phira clients do not support private room charts.**

```sh
cargo run --release --manifest-path vendor/phira-mp/Cargo.toml -p phira-mp-server -- --port 12346
```

Build the modified Phira app from the repository root as usual. Set **Multiplayer Server Address** to the host/port of your own server, then connect with your regular Phira account.

## Host a local chart

1. Import a chart into Phira's **Local** library first. Its chart directory must include a valid `info.yml` and referenced media.
2. Create or join a room on the matching custom MP server.
3. As **room host**, tap the imported local chart in the library. This uploads a compressed ZIP to the room's server and selects it, without publishing to the Phira website.
4. Start the room. Everyone (including the host) downloads and caches that chart as `mp-local/<sha256>` when they start/ready, then plays it together.
5. Late joiners can download the selected chart while the room is still in chart-selection state.

Official Phira chart IDs remain supported and keep their normal downloading and result-upload behavior.

## Limits and behavior

- Chart ZIP limit: **32 MiB** compressed. Chunk size: **64 KiB**, for the MP protocol's 2 MiB packet limit.
- Extraction limit: **128 MiB** total unpacked files and at most **4096** ZIP entries.
- Each chunk uses an ordered offset. The completed archive is checked with **SHA-256** by the server and by each receiver before extraction.
- The chart is stored **only in the multiplayer server's room memory**. The server retains it while the room exists; the game caches received files locally. This is *not* a Phira website upload.
- Only the room's host may upload or replace a chart during chart selection. Any authenticated player in the room may request the currently selected room chart.
- Local-chart scores are shown in the room and used to finish the match, but are **not sent to the official Phira website or ranked leaderboard**. They are not independently validated by the server.
- Multiplayer account authentication remains unchanged; this feature does not bypass website authentication for joining a room.
- You need to share the modified server address and distribute a compatible build to every participant. Opening the port externally depends on your network setup.

## Development checks

```sh
cargo fmt --all --check
cargo test --workspace
cargo check -p phira
cargo test --manifest-path vendor/phira-mp/Cargo.toml --workspace
```

This implementation needs end-to-end verification with two running modified clients and the modified server before production use. In particular, test ZIP extraction, network disconnection mid-upload, switching back to official charts, score completion, and replay of a selected local chart.

## Current cache policy

Currently, transferred charts are cached under `mp-local/<SHA-256>`. They aren’t added to the visible Local library. The hash identifies the downloaded content, so receiving the exact same archive can reuse its cached copy.

This is the archive SHA-256, not a catalog chart ID. The client still receives and verifies the ZIP before checking for an existing extracted cache folder; reuse avoids repeated extraction, but does not skip the network transfer or readiness wait. There is no Save to Local button and no new content-hash metadata protocol. The previously working private-chart server remains compatible. Authentication is unchanged.
