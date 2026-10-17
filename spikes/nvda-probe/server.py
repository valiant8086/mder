"""Tiny localhost server for the NVDA probe page.

Serves index.html and appends every event the page POSTs to /log as one
JSON line in logs/probe-<timestamp>.jsonl. Standard library only.

    python server.py            # then open http://127.0.0.1:8765/ in Edge
"""
import datetime
import http.server
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent
LOG_DIR = ROOT / "logs"
LOG_DIR.mkdir(exist_ok=True)
LOG_FILE = LOG_DIR / f"probe-{datetime.datetime.now():%Y%m%d-%H%M%S}.jsonl"
PORT = 8765


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

    def do_POST(self):
        if self.path != "/log":
            self.send_error(404)
            return
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        with open(LOG_FILE, "ab") as f:
            f.write(body.strip() + b"\n")
        self.send_response(204)
        self.end_headers()

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    print(f"Probe: http://127.0.0.1:{PORT}/")
    print(f"Log:   {LOG_FILE}")
    http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
