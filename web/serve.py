#!/usr/bin/env python3
"""Dev server for netlab — adds COOP/COEP so SharedArrayBuffer (emscripten
pthreads) is allowed."""
import http.server, sys

class H(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Cross-Origin-Resource-Policy', 'cross-origin')
        self.send_header('Cache-Control', 'no-cache')
        super().end_headers()
    def log_message(self, *a):
        pass

port = int(sys.argv[1]) if len(sys.argv) > 1 else 8471
http.server.ThreadingHTTPServer(('0.0.0.0', port), H).serve_forever()
