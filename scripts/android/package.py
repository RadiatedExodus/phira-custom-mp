"""Reuse the official Android shell, replacing the game library and tracked assets."""
from pathlib import Path
import os
import subprocess
import zipfile

library = Path('target/aarch64-linux-android/release/libphira.so')
ndk = Path(os.environ['ANDROID_NDK_HOME']) / 'toolchains/llvm/prebuilt/linux-x86_64'
nm = ndk / 'bin/llvm-nm'
symbols = subprocess.check_output([str(nm), '-D', '--defined-only', str(library)], text=True)
required = ['initializeEnvironment', 'preprocessInput', 'activityOnCreate',
            'initializeContext', 'surfaceOnTouch', 'processExportFd',
            'setDataPath', 'setTempDir', 'setChosenFile', 'markImport']
for name in required:
    assert 'Java_quad_1native_QuadNative_' + name in symbols, name

with zipfile.ZipFile('build/android/template.apk') as source:
    assets = {str(p).replace('\\', '/'): p for p in Path('assets').rglob('*') if p.is_file()}
    with zipfile.ZipFile('build/android/unsigned.apk', 'w') as target:
        for entry in source.infolist():
            if entry.filename.startswith('META-INF/') or entry.filename in assets:
                continue
            if entry.filename == 'lib/arm64-v8a/libphira.so':
                target.writestr(entry, library.read_bytes())
            else:
                target.writestr(entry, source.read(entry))
        for name, path in sorted(assets.items()):
            target.write(path, name, compress_type=zipfile.ZIP_DEFLATED)

with zipfile.ZipFile('build/android/unsigned.apk') as apk:
    assert apk.testzip() is None
    assert apk.read('lib/arm64-v8a/libphira.so') == library.read_bytes()
    assert 'assets/font.ttf' in apk.namelist()
    assert len(apk.namelist()) == len(set(apk.namelist()))
