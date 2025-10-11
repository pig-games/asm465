#!/usr/bin/env python3
"""Wait for a TCP port to become reachable."""

from __future__ import annotations

import argparse
import socket
import sys
import time


def wait_for_port(host: str, port: int, retries: int, delay: float) -> bool:
    for _ in range(retries):
        with socket.socket() as sock:
            sock.settimeout(delay)
            try:
                sock.connect((host, port))
            except OSError:
                time.sleep(delay)
            else:
                return True
    return False


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Wait until host:port becomes reachable.")
    parser.add_argument("--host", default="127.0.0.1", help="Target host (default: 127.0.0.1)")
    parser.add_argument("--port", type=int, required=True, help="Target port")
    parser.add_argument(
        "--retries",
        type=int,
        default=10,
        help="Number of attempts before failing (default: 10)",
    )
    parser.add_argument(
        "--delay",
        type=float,
        default=1.0,
        help="Delay in seconds between attempts (default: 1.0)",
    )
    args = parser.parse_args(argv)

    if wait_for_port(args.host, args.port, args.retries, args.delay):
        return 0

    print(f"{args.host}:{args.port} did not become ready in time", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
