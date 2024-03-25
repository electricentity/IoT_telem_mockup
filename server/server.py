from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from datetime import datetime

class SimpleHTTPRequestHandler(BaseHTTPRequestHandler):

    def do_POST(self):
        content_length = int(self.headers['Content-Length'])
        post_data = self.rfile.read(content_length)

        try:
            data = json.loads(post_data.decode('utf-8'))
            data['received_at'] = datetime.now().isoformat() + 'Z'
            print(json.dumps(data))
            self.send_response(200)
            self.end_headers()
            response = bytes(json.dumps({'status': 'success'}), 'utf-8')
            self.wfile.write(response)
        except json.JSONDecodeError:
            self.send_response(400)
            self.end_headers()
            response = bytes(json.dumps({'status': 'fail', 'message': 'Invalid JSON'}), 'utf-8')
            self.wfile.write(response)

    def log_message(self, format, *args):
        # Override to prevent logging of requests as we are handling in the POST
        pass

def run(server_class=HTTPServer, handler_class=SimpleHTTPRequestHandler, port=8080):
    server_address = ('', port)
    httpd = server_class(server_address, handler_class)
    print(f'Starting httpd on port {port}...')
    httpd.serve_forever()

if __name__ == "__main__":
    run()
