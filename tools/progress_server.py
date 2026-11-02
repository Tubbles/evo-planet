#!/usr/bin/env python3
"""Tiny progress page for update-and-run.sh.

Serves a full-screen page that tails a build log and redirects to the game
URL once the log contains the line "SERVER READY". Standard library only.

    progress_server.py <logfile> <port> <game_url>
"""
import html
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOG, PORT, GAME_URL = sys.argv[1], int(sys.argv[2]), sys.argv[3]

PAGE = """<!doctype html><html><head><meta charset="utf-8"><title>Evo Planet · updating</title>
<style>
html,body{margin:0;height:100%;background:#0f1613;color:#e4ebe6;font:22px/1.4 "IBM Plex Sans",system-ui,sans-serif}
main{max-width:1100px;margin:0 auto;padding:6vh 5vw;display:grid;gap:3vh}
h1{font-size:48px;margin:0;letter-spacing:-0.01em}
.state{font-size:28px;color:#e3a040;display:flex;align-items:center;gap:16px}
.spin{width:26px;height:26px;border:4px solid #2c3a33;border-top-color:#e3a040;border-radius:50%;animation:s 1s linear infinite}
@keyframes s{to{transform:rotate(360deg)}}
.ok .spin{animation:none;border-color:#6cc37c}.fail .spin{animation:none;border-color:#e08a5a}.fail .state{color:#e08a5a}
pre{background:#151e1a;border:1px solid #2c3a33;border-radius:12px;padding:20px;font:17px/1.35 "IBM Plex Mono",monospace;height:52vh;overflow:hidden;white-space:pre-wrap;word-break:break-word;margin:0}
.hint{color:#94a59c;font-size:18px}
</style></head><body><main>
<h1>Evo Planet</h1>
<div class="state"><span class="spin"></span><span id="state">Updating and building…</span></div>
<pre id="log"></pre>
<p class="hint">The game opens here automatically when the build finishes. Hold the Guide button to return to Steam.</p>
</main><script>
const GAME = %GAME%;
async function poll(){
  try{
    const r = await fetch('/log?ts='+Date.now()); const j = await r.json();
    document.getElementById('log').textContent = j.tail;
    const st = document.getElementById('state');
    if (j.ready){ document.body.className='ok'; st.textContent='Ready, launching…'; setTimeout(()=>{ location.href = GAME; }, 800); return; }
    if (j.failed){ document.body.className='fail'; st.textContent='Build failed. See the log; the previous build will start if one exists.'; }
    else { st.textContent = j.phase || 'Working…'; }
  }catch(e){}
  setTimeout(poll, 1000);
}
poll();
</script></body></html>"""


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):  # quiet
        pass

    def _send(self, code, ctype, body):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path.startswith("/log"):
            try:
                with open(LOG, "r", errors="replace") as f:
                    lines = f.read().splitlines()
            except OSError:
                lines = []
            ready = any("SERVER READY" in l for l in lines)
            failed = any("BUILD FAILED" in l for l in lines)
            phase = next((l.split("] ", 1)[-1] for l in reversed(lines) if l.startswith("[") and "] " in l), "")
            tail = "\n".join(lines[-40:])
            import json
            self._send(200, "application/json", json.dumps({"tail": tail, "ready": ready, "failed": failed, "phase": phase}).encode())
        else:
            import json
            self._send(200, "text/html; charset=utf-8", PAGE.replace("%GAME%", json.dumps(GAME_URL)).encode())


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
