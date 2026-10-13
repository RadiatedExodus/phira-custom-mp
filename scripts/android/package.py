"""Reuse the official Android shell, replacing the game library and tracked assets."""
from pathlib import Path
import os
import subprocess
import struct
import zipfile


def set_min_sdk(data, version):
    """Change only the typed minSdkVersion integer in Android binary XML."""
    data = bytearray(data)
    strings = []
    cursor = 8
    changed = False
    while cursor < len(data):
        kind, header_size, size = struct.unpack_from('<HHI', data, cursor)
        assert size >= header_size and cursor + size <= len(data)
        if kind == 1:  # RES_STRING_POOL_TYPE
            count, _, flags, start = struct.unpack_from('<IIII', data, cursor + 8)
            for i in range(count):
                offset = struct.unpack_from('<I', data, cursor + header_size + i * 4)[0]
                pos = cursor + start + offset
                if flags & 0x100:
                    pos += 2 if data[pos] & 0x80 else 1
                    length = data[pos]
                    pos += 1
                    if length & 0x80:
                        length = ((length & 0x7f) << 8) | data[pos]
                        pos += 1
                    strings.append(data[pos:pos + length].decode('utf-8'))
                else:
                    length = struct.unpack_from('<H', data, pos)[0]
                    pos += 2
                    if length & 0x8000:
                        length = ((length & 0x7fff) << 16) | struct.unpack_from('<H', data, pos)[0]
                        pos += 2
                    strings.append(data[pos:pos + length * 2].decode('utf-16le'))
        elif kind == 0x102:  # RES_XML_START_ELEMENT_TYPE
            extension = cursor + header_size
            name = struct.unpack_from('<I', data, extension + 4)[0]
            if strings[name] == 'uses-sdk':
                start, width, count = struct.unpack_from('<HHH', data, extension + 8)
                for i in range(count):
                    pos = extension + start + i * width
                    name = struct.unpack_from('<I', data, pos + 4)[0]
                    if strings[name] == 'minSdkVersion':
                        assert data[pos + 15] == 0x10
                        struct.pack_into('<I', data, pos + 16, version)
                        changed = True
        cursor += size
    assert changed, 'minSdkVersion not found'
    return bytes(data)

library = Path('target/aarch64-linux-android/release/libphira.so')
ndk = Path(os.environ['ANDROID_NDK_HOME']) / 'toolchains/llvm/prebuilt/linux-x86_64'
nm = ndk / 'bin/llvm-nm'
symbols = subprocess.check_output([str(nm), '-D', '--defined-only', str(library)], text=True)
required = ['activityOnCreate', 'activityOnDestroy', 'activityOnPause', 'activityOnResume',
            'initializeContext', 'initializeEnvironment', 'markImport', 'markImportRespack',
            'preprocessInput', 'processExportFd', 'prprActivityOnDestroy',
            'prprActivityOnPause', 'prprActivityOnResume', 'releaseContext',
            'setChosenFile', 'setDataPath', 'setDpi', 'setTempDir',
            'surfaceOnSurfaceChanged', 'surfaceOnSurfaceCreated',
            'surfaceOnSurfaceDestroyed', 'surfaceOnTouch']
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
            elif entry.filename == 'lib/arm64-v8a/libc++_shared.so':
                target.writestr(entry, (ndk / 'sysroot/usr/lib/aarch64-linux-android/libc++_shared.so').read_bytes())
            elif entry.filename == 'AndroidManifest.xml':
                target.writestr(entry, set_min_sdk(source.read(entry), 26))
            else:
                target.writestr(entry, source.read(entry))
        for name, path in sorted(assets.items()):
            target.write(path, name, compress_type=zipfile.ZIP_DEFLATED)

with zipfile.ZipFile('build/android/unsigned.apk') as apk:
    assert apk.testzip() is None
    assert apk.read('lib/arm64-v8a/libphira.so') == library.read_bytes()
    assert 'assets/font.ttf' in apk.namelist()
    assert len(apk.namelist()) == len(set(apk.namelist()))

