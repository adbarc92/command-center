"""lock.py — cross-platform exclusive advisory lock on a dedicated .lock file.

Windows: msvcrt.locking on a 1-byte region. POSIX: fcntl.flock. Bounded retry/backoff so an
append never hangs a session; on exhaustion raises LockTimeout (callers decide skip vs preserve).
"""
from __future__ import annotations

import contextlib
import time
from pathlib import Path

try:
    import msvcrt  # Windows
    _WINDOWS = True
except ImportError:  # POSIX
    import fcntl
    _WINDOWS = False


class LockTimeout(Exception):
    pass


@contextlib.contextmanager
def file_lock(lock_path: Path, tries: int = 10, backoff: float = 0.2):
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    fh = open(lock_path, "a+b")
    try:
        acquired = False
        for attempt in range(tries):
            try:
                if _WINDOWS:
                    fh.seek(0)
                    msvcrt.locking(fh.fileno(), msvcrt.LK_NBLCK, 1)
                else:
                    fcntl.flock(fh.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                acquired = True
                break
            except OSError:
                time.sleep(backoff)
        if not acquired:
            raise LockTimeout(f"could not lock {lock_path} after {tries} tries")
        yield
    finally:
        try:
            if _WINDOWS:
                fh.seek(0)
                msvcrt.locking(fh.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(fh.fileno(), fcntl.LOCK_UN)
        except OSError:
            pass
        fh.close()
