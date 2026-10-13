# Windows private-chart crossplay test

Extract the whole ZIP into a writable folder and run Start-Phira.cmd.
Keep Phira-Private-Charts.exe next to assets. This is an unsigned, optimized Windows x64
build with video support, built from the multiplayer-host-local-charts branch.

Connect to the same modified Linux MP server as the Android client. Log in with
a separate normal Phira account on each device, join the same room, and have the
room host select an imported local chart. All participants need compatible
modified clients; an upstream MP server does not support private charts.
Test Windows hosting and Android joining, then reverse the roles.

The multiplayer protocol is unchanged from Android build dbf9386.
Login/authentication remains unchanged. Selection uploads the chart; starting
downloads and verifies it for each player, so some delay is expected.
Private scores are room-only. Official score submission may be unavailable
because the public build lacks the closed official attestation code.

The workflow verifies compilation, required assets and executable architecture.
Cross-platform gameplay still requires testing on the actual devices.
The assets missing from the source repository come from the checksum-pinned
official v0.8.2 Windows package; its executable is not used.

Imported charts under custom/ now use private transfer even when their metadata
contains a catalog ID. Downloaded catalog charts keep normal online selection.
Built-in charts cannot be transferred: import a chart ZIP into Local first.
Check Settings > version against source-commit.txt to confirm this build.
