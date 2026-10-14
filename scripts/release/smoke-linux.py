"""Check startup only; this does not claim interactive gameplay coverage."""
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time


def stop(process):
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()


with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)
    for name, source in zip(('client', 'server'), sys.argv[1:]):
        shutil.copytree(source, root / name)
    env = dict(os.environ, LIBGL_ALWAYS_SOFTWARE='1')
    with (root / 'client.log').open('w+') as log:
        process = subprocess.Popen(['xvfb-run', '-a', './Start-Phira.sh'], cwd=root / 'client', env=env,
                                   stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            time.sleep(15)
            if process.poll() is not None:
                raise RuntimeError(f'Linux client exited early: {process.returncode}')
            print('Linux client remained running for 15 seconds under Xvfb; gameplay not tested.')
        finally:
            stop(process)
            log.seek(0)
            print(log.read())
    with (root / 'server.log').open('w+') as log:
        process = subprocess.Popen(['./phira-mp-server', '--port', '12346'], cwd=root / 'server',
                                   stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            for _ in range(50):
                if process.poll() is not None:
                    raise RuntimeError(f'Linux server exited early: {process.returncode}')
                try:
                    with socket.create_connection(('::1', 12346), timeout=1):
                        break
                except OSError:
                    time.sleep(.2)
            else:
                raise RuntimeError('Linux server did not open its listening port')
            print('Linux server opened TCP port 12346; authenticated room play not tested.')
        finally:
            stop(process)
            log.seek(0)
            print(log.read())
