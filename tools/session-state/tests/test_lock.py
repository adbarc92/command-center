import threading
import time
from pathlib import Path
import session_state.lock as L


def test_lock_serializes_writers(tmp_path):
    lock = tmp_path / "t.lock"
    target = tmp_path / "out.txt"
    target.write_text("", encoding="utf-8")
    order = []

    def worker(tag):
        with L.file_lock(lock):
            cur = target.read_text(encoding="utf-8")
            time.sleep(0.05)  # widen the race window
            target.write_text(cur + tag + "\n", encoding="utf-8")
            order.append(tag)

    threads = [threading.Thread(target=worker, args=(str(i),)) for i in range(5)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    lines = [l for l in target.read_text(encoding="utf-8").splitlines() if l]
    assert len(lines) == 5  # no lost updates → all writers serialized


def test_lock_timeout_raises(tmp_path):
    lock = tmp_path / "t.lock"
    with L.file_lock(lock):
        raised = []

        def contender():
            try:
                with L.file_lock(lock, tries=2, backoff=0.01):
                    pass
            except L.LockTimeout:
                raised.append(True)

        t = threading.Thread(target=contender)
        t.start()
        t.join()
        assert raised == [True]
