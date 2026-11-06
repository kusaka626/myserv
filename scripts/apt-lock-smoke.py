"""Run only inside a disposable Ubuntu container with python3 installed.

Uses the installer's apt options and real dpkg locks; no package upgrades occur.
"""
import fcntl
import pathlib
import shlex
import subprocess
import time

installer = pathlib.Path('/workspace/myserv/infra/install.sh').read_text()
line = next(s for s in installer.splitlines() if s.startswith('apt-get -o '))
args = shlex.split(line)
prefix = args[:args.index('install')]
assert 'DPkg::Lock::Timeout=600' in prefix
version = subprocess.check_output(['dpkg-query', '-W', '-f=${Version}', 'base-files'], text=True)
command = prefix + ['install', '-y', '--no-download', 'base-files=' + version]
for name in ['lock-frontend', 'lock']:
    path = pathlib.Path('/var/lib/dpkg') / name
    with path.open('a') as lock:
        fcntl.lockf(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        inode = path.stat().st_ino
        child = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            time.sleep(2)
            assert child.poll() is None, 'apt exited instead of waiting for the lock'
            assert path.stat().st_ino == inode, 'lock file must not be removed'
            fcntl.lockf(lock, fcntl.LOCK_UN)
            output, _ = child.communicate(timeout=30)
            assert child.returncode == 0, output
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
    print(f'Ubuntu apt: waits for {name}, resumes after release: PASS')
