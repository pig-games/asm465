#!/usr/bin/env python3
"""
Send a `run_prg` request to the asm465 service API.

Usage: send_prg.py <prg_path> <port> <max_cycles> [--host HOST] [options]
"""

from __future__ import annotations

import argparse
import base64
import json
import pathlib
import socket
import sys


def _send_request(request: dict, host: str, port: int) -> str:
    with socket.create_connection((host, port), timeout=10) as conn:
        conn.sendall((json.dumps(request) + "\n").encode("utf-8"))
        response_line = conn.makefile().readline()

    if not response_line:
        raise RuntimeError("no response from asm465 service")

    response = json.loads(response_line)
    status = response.get("status", "error")
    message = response.get("message", "")

    if status != "ok":
        raise RuntimeError(message or "asm465 service returned error status")

    return message


def send_run_prg(
    prg_path: pathlib.Path,
    host: str,
    port: int,
    max_cycles: int,
    start: int | None,
) -> str:
    request = {
        "cmd": "run_prg",
        "path": str(prg_path.resolve()),
        "max_cycles": max_cycles,
    }
    if start is not None:
        request["start"] = start
    return _send_request(request, host, port)

def send_run_prg_data(
    prg_path: pathlib.Path,
    host: str,
    port: int,
    max_cycles: int,
    start: int | None,
    name: str | None,
) -> str:
    data = prg_path.read_bytes()
    request = {
        "cmd": "run_prg_data",
        "data": base64.b64encode(data).decode("ascii"),
        "max_cycles": max_cycles,
    }
    if start is not None:
        request["start"] = start
    if name:
        request["name"] = name
    else:
        request["name"] = prg_path.name or "inline.prg"
    return _send_request(request, host, port)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Send run_prg to asm465 service")
    parser.add_argument("prg_path", type=pathlib.Path, help="Path to the PRG to execute")
    parser.add_argument("port", type=int, help="Service port")
    parser.add_argument("max_cycles", type=int, help="Maximum cycles to run the program")
    parser.add_argument(
        "--start",
        type=lambda v: int(v, 0),
        default=None,
        help="Optional start address override (decimal/0x??)",
    )
    parser.add_argument(
        "--embed",
        action="store_true",
        help="Embed the binary data directly in the request",
    )
    parser.add_argument(
        "--name",
        help="Optional display name when using --embed",
    )
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Service host (default: 127.0.0.1)",
    )
    args = parser.parse_args(argv)

    try:
        if args.embed:
            message = send_run_prg_data(
                args.prg_path,
                args.host,
                args.port,
                args.max_cycles,
                args.start,
                args.name,
            )
        else:
            message = send_run_prg(
                args.prg_path,
                args.host,
                args.port,
                args.max_cycles,
                args.start,
            )
    except Exception as exc:  # noqa: B902 - broad for CLI error reporting
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    if message:
        print(message)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
