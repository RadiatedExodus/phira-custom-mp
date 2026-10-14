# Phira — private multiplayer chart sharing

This fork of [TeamFlos/Phira](https://github.com/TeamFlos/phira) lets a room host share an imported chart privately with other players through a matching self-hosted multiplayer server. Windows–Android crossplay has been reported working by the user. This is an experimental community build; login and account authentication are unchanged.

> **Score limitation:** these public-source Windows and Android builds cannot submit official scores, including scores for Phira catalogue charts. They lack the closed official score-record/attestation implementation. Private multiplayer scores are displayed in the room only. Use the official client for official scores and RKS. See the [upstream Android build guide](https://teamflos.github.io/phira-docs/phira_build_guide/Android.html).

## Download this fork

The following builds contain the current private-chart implementation at commit `ccb434b`:

| Platform | Tested build artifact | Installation |
| --- | --- | --- |
| Windows x64 | [Download Windows ZIP](https://github.com/shinova55/phira-custom-mp/actions/runs/38521738137/artifacts/11873804752) | Extract the whole ZIP into a writable folder and run `Start-Phira.cmd`. Keep the executable beside `assets`. |
| Android ARM64, Android 8+ | [Download Android APK artifact](https://github.com/shinova55/phira-custom-mp/actions/runs/38521738038/artifacts/11873988578) | Extract the artifact ZIP and install `phira-private-charts-arm64-test.apk`. |

GitHub sign-in may be required. These artifacts expire on November 13, 2026; subsequent builds are available through [GitHub Actions](https://github.com/shinova55/phira-custom-mp/actions). Windows is unsigned. Android uses an ephemeral test certificate and the official package ID, so it cannot update an official installation or a build signed with a different key. Back up/export local data before reinstalling, or use a separate Android user/profile. No release-signing secrets are needed.

See the [Windows notes](docs/windows-test-build.md) and [Android notes](docs/android-test-build.md) for build and installation details. Build checks and package verification passed; automated builds do not test multiplayer gameplay on devices.

## Share a private chart

1. Run the matching server described below and give its address to every player.
2. Each player uses this modified client, logs in with their normal Phira account, and connects to the same server and room.
3. The room host imports a chart into **Local**, then selects that imported chart in the room. Imported charts use private transfer even if their metadata contains a catalogue ID.
4. Selection uploads the chart to the room server. Starting/readying downloads and verifies it for each participant, including the host.

Charts are shared with members of that room without publishing them to the Phira catalogue. Downloaded catalogue charts retain normal catalogue selection; built-in charts must first be imported as a chart ZIP to use private sharing.

## Cache and practical limitations

Currently, transferred charts are cached under `mp-local/<SHA-256>`. They aren’t added to the visible Local library. The hash identifies the downloaded content, so receiving the exact same archive can reuse its cached copy.

- There is no **Save to Local** feature. Receiving a chart does not add a duplicate entry to the visible library or make it available there for solo play.
- The hash identifies archive bytes, not a catalogue level ID. Repacking a chart can create a different cache folder. Cached files can remain on disk after a match; there is no bounded cache eviction policy.
- The client still downloads and verifies the ZIP before looking for an existing extracted folder. Cache reuse saves extraction work, but does **not** skip transfer or readiness waits. Selection and starting can therefore take time even when players have played the chart before.
- Archives are limited to **32 MiB** compressed, **128 MiB** extracted, and **4096** ZIP entries. Transfers use ordered **64 KiB** chunks and SHA-256 verification.
- Private scores are room-only and are not independently validated by the server. Official score submission is unavailable in these builds, including for catalogue charts.
- All players need compatible modified clients and the matching modified server. A normal upstream MP server may allow login and room entry but does not support private chart transfer; it can time out with “deadline has elapsed.”

## Server structure and setup

The repository includes the modified multiplayer components in a separate Rust workspace:

| Directory | Role |
| --- | --- |
| `phira/` | Game client, chart selection and multiplayer UI |
| `vendor/phira-mp/phira-mp-client/` | Client connection and transfer handling |
| `vendor/phira-mp/phira-mp-common/` | Shared protocol and commands |
| `vendor/phira-mp/phira-mp-server/` | Multiplayer rooms and private chart relay |

```text
Host imports/selects a chart
    -> uploads ZIP to the custom MP server's room memory
    -> room members (including host) download and verify it
    -> each client caches it under mp-local/<SHA-256> and plays
```

This is a server-relayed transfer. The server holds the selected archive in room memory while the room exists; it does not publish it to the catalogue or keep a permanent chart library. Only the room host can upload/replace it during chart selection. Authenticated room members can download the current chart. Normal Phira account authentication remains required.

On Linux, with the repository's Rust toolchain and build dependencies installed, run from the repository root:

```sh
cargo run --release --manifest-path vendor/phira-mp/Cargo.toml -p phira-mp-server -- --port 12346
```

Allow access to the server port as appropriate for your network, then set **Multiplayer Server Address** in each client to your server's host and port. Build the server from this repository's `vendor/phira-mp`; installing an upstream MP server separately will not provide this feature. The server check workflow checks/tests the code but does not publish a Linux server binary.

See [private multiplayer details](docs/multiplayer-host-local-charts.md) for limits and development checks. Basic private sharing and Windows–Android crossplay have been user-tested, but disconnect recovery, late joins and all device configurations are not comprehensively verified.

## Upstream Phira

![Phira Showcase](https://phira.5wyxi.com/files/github-showcase.png)

[中文版本](./README-zh_CN.md)

## Official upstream downloads

- [GitHub Release](https://github.com/TeamFlos/phira/releases): For Android, Windows and Linux
- [616.sb mirror](https://616.sb/#phira): Android only
- [App Store](https://apps.apple.com/us/app/phira/id6447435864): For iOS
- ~~[.ipa Release](https://github.com/F-Unction/phira_ipa/releases): For iOS~~

## Contributions

For this fork, use [this repository’s issues](https://github.com/shinova55/phira-custom-mp/issues) and pull requests. The following links and contact information belong to the upstream Phira project.

Issues & pull requests are welcome! Also check these good first [issues](https://github.com/TeamFlos/phira/issues?q=label%3A%22good+first+issue%22) and [pull requests](https://github.com/TeamFlos/phira/issues?q=label%3A%22good+first+issue%22).

If you want to talk with us in depth, email [contact@phira.cn](mailto://contact@phira.cn). Currently we are looking for **Rust** Programmers *with rhythm/music game experience*.

## Translation

See our [Weblate](https://hosted.weblate.org/engage/phira/) page for the latest translation progress. You can easily contribute to translations there.

![Translation Progress](https://hosted.weblate.org/widget/phira/svg-badge.svg)

![Translation Status](https://hosted.weblate.org/widget/phira/multi-auto.svg)

## Star History

[![Stargazers over time](https://starchart.cc/TeamFlos/phira.svg?variant=adaptive)](https://starchart.cc/TeamFlos/phira)
