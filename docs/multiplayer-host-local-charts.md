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
4. Start the room. Clients fetch small metadata first and verify an existing cached or saved copy. Only clients missing matching content download it to `mp-local/<content-sha256>`. The host reuses its imported copy.
5. After returning from private-chart play, choose **Save to Local** in the room panel to keep the last played chart for solo play. Repeated saves reuse identical library content.

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

## Verified caching and optional saving

Update the server **and all clients** to this cache-enabled version. The new metadata/upload commands are appended so existing command numbers are preserved. An older private-chart client can still upload/download on the updated server, but its uploads lack a content hash and therefore require downloading before verification. New clients require the updated server for private charts; authentication and official charts are unchanged.

- Content identity is SHA-256 over sorted extracted file paths, lengths, and each file's SHA-256, with the `phira-private-chart-files-v1` domain prefix. ZIP timestamps, compression, and file ordering do not affect identity. Changed metadata or media count as a different version. The archive has a separate SHA-256 integrity check.
- Cache hits are rehashed before reuse. The cache is checked first; on a miss, existing local-library directories are searched and verified. The initial library scan can take time on devices with many charts. Caching avoids transfer and extraction, but loading, readiness, and waiting for other players still take time. Host selection still uploads the ZIP to the room server.
- The temporary cache is limited to **512 MiB of extracted file content**. Before installation, the oldest unused entries are removed; the current chart is protected. Interrupted extraction folders are removed before the next installation. Saved charts under `custom/` or `download/` are never evicted. A single expanded chart remains limited to 128 MiB.
- Save to Local moves a verified cache entry into `custom/mp-<content-hash>` and adds a local library entry with no catalog ID. It reuses an identical saved chart and preserves its records. Saving never publishes a chart or sends its multiplayer score to the catalog. A changed local copy is retained as a separate version.
- Integrity, extraction, cache cleanup, and library persistence errors are shown instead of being treated as success. If files were saved but writing the library failed, retry Save to Local. A server timeout still requires checking connectivity/server version.

### Linux server artifact

The Custom multiplayer server workflow publishes `phira-mp-linux-x64-<commit>` (Ubuntu 22.04 x86_64 build). Extract its tar.gz, stop your old server, and run `./phira-mp-server --port 12346` from a writable directory. Keep your existing port/service configuration. ARM Linux servers must build from source with the command above. This update is not deployed automatically to your server.
