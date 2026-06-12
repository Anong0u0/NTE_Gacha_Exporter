from __future__ import annotations

import contextlib
import platform
import select
import signal
import sys
import threading
import time
from typing import Any, TextIO


def wait_for_q(stop_event: threading.Event, stream: TextIO) -> None:
    if platform.system() == "Windows":
        import msvcrt

        while not stop_event.is_set():
            try:
                key = msvcrt.getwch() if msvcrt.kbhit() else ""
            except KeyboardInterrupt:
                stop_event.set()
                return
            if key.lower() == "q":
                stop_event.set()
                return
            time.sleep(0.1)
        return

    try:
        while not stop_event.is_set():
            readable, _writable, _errors = select.select([stream], [], [], 0.1)
            if readable and stream.read(1).lower() == "q":
                stop_event.set()
                return
    except KeyboardInterrupt:
        stop_event.set()


def start_q_listener(stop_event: threading.Event, stream: TextIO | None = None) -> threading.Thread | None:
    actual_stream = stream or sys.stdin
    if not actual_stream.isatty():
        return None
    thread = threading.Thread(target=wait_for_q, args=(stop_event, actual_stream), daemon=True)
    thread.start()
    return thread


def install_sigint_stop(stop_event: threading.Event) -> Any:
    try:
        previous = signal.getsignal(signal.SIGINT)

        def handle_sigint(signum: int, frame: Any) -> None:
            if stop_event.is_set() and callable(previous):
                previous(signum, frame)
                return
            stop_event.set()

        signal.signal(signal.SIGINT, handle_sigint)
    except ValueError:
        return None
    return previous


def restore_sigint_handler(previous: Any) -> None:
    if previous is None:
        return
    with contextlib.suppress(ValueError):
        signal.signal(signal.SIGINT, previous)
