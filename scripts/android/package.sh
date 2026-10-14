#!/usr/bin/env bash
set -euo pipefail
mkdir -p build/android dist
curl --fail --location --retry 3 \
  https://github.com/TeamFlos/phira/releases/download/v0.8.2/Phira-android-arm64-v8a-v0.8.2.apk \
  -o build/android/template.apk
echo 'b6ee79958c21bc66187b8d8697257617eedd440af5b042a7b8920cc297bef3b0  build/android/template.apk' | sha256sum --check
python3 scripts/android/package.py
zipalign -P 16 -f 4 build/android/unsigned.apk build/android/aligned.apk
# An ephemeral, public-password test key; no repository signing secrets are used.
keytool -genkeypair -keystore build/android/test.keystore -storepass android \
  -keypass android -alias androiddebugkey -dname 'CN=Android Debug,O=Android,C=US' \
  -keyalg RSA -keysize 2048 -validity 3650
apksigner sign --ks build/android/test.keystore --ks-key-alias androiddebugkey \
  --ks-pass pass:android --key-pass pass:android \
  --out dist/phira-private-charts-arm64-test.apk build/android/aligned.apk
apksigner verify --verbose --print-certs dist/phira-private-charts-arm64-test.apk | tee dist/signature.txt
zipalign -c -P 16 -v 4 dist/phira-private-charts-arm64-test.apk > dist/alignment.txt
aapt dump badging dist/phira-private-charts-arm64-test.apk > dist/package.txt
git rev-parse HEAD > dist/source-commit.txt
sha256sum dist/*.apk > dist/SHA256SUMS.txt
cp docs/android-test-build.md dist/README.md
