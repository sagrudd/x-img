#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Prove image replacement and fail-open behavior in an installed Firefox.

All image bytes and browser state are ephemeral. Rust API tests separately
prove the authenticated HTTPS delivery header contract; this loopback harness
exercises Firefox CSP, CORS, CORP, MIME/length/ETag validation, blob display,
and restoration without installing an extension or retaining browsing data.
"""

from __future__ import annotations

import http.server
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from typing import ClassVar

IMAGE = b'<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="green"/></svg>'
CHECKSUM = "sha256:" + "a" * 64


class Evidence:
    events: set[str] = set()


class ObjectHandler(http.server.BaseHTTPRequestHandler):
    page_origin: ClassVar[str]

    def log_message(self, _format: str, *_args: object) -> None:
        pass

    def do_GET(self) -> None:  # noqa: N802
        mode = self.path.removeprefix("/")
        if mode not in {"good", "bad-type", "bad-length", "bad-etag", "bad-cors"}:
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("content-type", "text/plain" if mode == "bad-type" else "image/svg+xml")
        self.send_header("content-length", str(len(IMAGE) + (1 if mode == "bad-length" else 0)))
        self.send_header("etag", '"wrong"' if mode == "bad-etag" else f'"{CHECKSUM}"')
        if mode != "bad-cors":
            self.send_header("access-control-allow-origin", self.page_origin)
            self.send_header("access-control-allow-credentials", "true")
            self.send_header("access-control-expose-headers", "etag, content-length")
        self.send_header("cross-origin-resource-policy", "cross-origin")
        self.send_header("cache-control", "private, no-store")
        self.end_headers()
        self.wfile.write(IMAGE)


class PageHandler(http.server.BaseHTTPRequestHandler):
    object_origin: ClassVar[str]

    def log_message(self, _format: str, *_args: object) -> None:
        pass

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/event":
            self.send_error(404)
            return
        size = int(self.headers.get("content-length", "0"))
        Evidence.events.add(self.rfile.read(size).decode("ascii", "ignore"))
        self.send_response(204)
        self.end_headers()

    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/origin.svg":
            self.send_response(200)
            self.send_header("content-type", "image/svg+xml")
            self.send_header("content-length", str(len(IMAGE)))
            self.end_headers()
            self.wfile.write(IMAGE)
            return
        if self.path not in {"/probe", "/csp-frame"}:
            self.send_error(404)
            return
        connect = "'self'" if self.path == "/csp-frame" else f"'self' {self.object_origin}"
        modes = [] if self.path == "/csp-frame" else ["good", "bad-type", "bad-length", "bad-etag", "bad-cors"]
        script = f"""
const expectedType='image/svg+xml', expectedLength={len(IMAGE)}, checksum='{CHECKSUM}';
async function attempt(mode) {{
  const image=document.getElementById('candidate'), original=image.src; let blob;
  try {{
    const response=await fetch('{self.object_origin}/'+mode, {{credentials:'include',cache:'no-store',redirect:'error'}});
    const length=Number(response.headers.get('content-length'));
    if(!response.ok || response.headers.get('content-type')!==expectedType || length!==expectedLength || response.headers.get('etag')!==`\"${{checksum}}\"`) throw Error();
    const bytes=await response.arrayBuffer(); if(bytes.byteLength!==length) throw Error();
    blob=URL.createObjectURL(new Blob([bytes],{{type:expectedType}}));
    await new Promise((ok,bad)=>{{image.onload=ok;image.onerror=bad;image.src=blob;}});
    URL.revokeObjectURL(blob); return mode==='good'?'good':`unexpected-${{mode}}`;
  }} catch(_) {{ if(blob)URL.revokeObjectURL(blob); image.src=original; return mode==='good'?'unexpected-good':mode; }}
}}
(async()=>{{for(const mode of {modes!r}){{const result=await attempt(mode);fetch('/event',{{method:'POST',body:result}});}}
if({str(self.path == '/csp-frame').lower()}){{const result=await attempt('good');fetch('/event',{{method:'POST',body:result==='good'?'unexpected':'csp'}});}}}})();
"""
        frame = "<iframe src=/csp-frame></iframe>" if self.path == "/probe" else ""
        page = f"<!doctype html><meta charset=utf-8><meta http-equiv=Content-Security-Policy content=\"default-src 'self'; img-src 'self' blob:; connect-src {connect}; script-src 'unsafe-inline'; frame-src 'self'\"><img id=candidate src=/origin.svg>{frame}<script>{script}</script>".encode()
        self.send_response(200)
        self.send_header("content-type", "text/html; charset=utf-8")
        self.send_header("content-length", str(len(page)))
        self.end_headers()
        self.wfile.write(page)


def firefox_binary() -> str:
    macos = "/Applications/Firefox.app/Contents/MacOS/firefox"
    found = macos if os.path.exists(macos) else shutil.which("firefox")
    if not found:
        raise FileNotFoundError("Firefox was not found")
    return found


def main() -> int:
    objects = http.server.ThreadingHTTPServer(("127.0.0.1", 0), ObjectHandler)
    pages = http.server.ThreadingHTTPServer(("127.0.0.1", 0), PageHandler)
    page_origin = f"http://127.0.0.1:{pages.server_port}"
    object_origin = f"http://127.0.0.1:{objects.server_port}"
    ObjectHandler.page_origin = page_origin
    PageHandler.object_origin = object_origin
    for server in (objects, pages):
        threading.Thread(target=server.serve_forever, daemon=True).start()
    profile = tempfile.mkdtemp(prefix="x-img-firefox-images-")
    browser: subprocess.Popen[bytes] | None = None
    required = {"good", "bad-type", "bad-length", "bad-etag", "bad-cors", "csp"}
    try:
        browser = subprocess.Popen([
            firefox_binary(), "--headless", "--no-remote", "--profile", profile,
            f"{page_origin}/probe",
        ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            unexpected = sorted(event for event in Evidence.events if event.startswith("unexpected"))
            if unexpected:
                raise RuntimeError(f"Firefox accepted invalid substitution evidence: {unexpected}")
            if required.issubset(Evidence.events):
                print("Firefox image substitution passed: display plus CSP/CORS/CORP/type/length/ETag fail-open")
                return 0
            time.sleep(0.1)
        raise RuntimeError(f"Firefox evidence timed out; missing: {sorted(required - Evidence.events)}")
    finally:
        if browser and browser.poll() is None:
            browser.terminate()
            try:
                browser.wait(timeout=5)
            except subprocess.TimeoutExpired:
                browser.kill()
        for server in (objects, pages):
            server.shutdown()
            server.server_close()
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (FileNotFoundError, RuntimeError) as error:
        print(f"Firefox image substitution failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
