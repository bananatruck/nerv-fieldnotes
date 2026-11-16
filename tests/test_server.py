"""Exercise real HTTP, disk persistence, and the CLI against an isolated runtime."""
import http.client
import importlib.util
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('fieldnotes_server', ROOT / 'server.py')
server_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(server_module)


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.static = self.root / 'dist'
        self.static.mkdir()
        (self.static / 'index.html').write_text('<h1>Fieldnotes test</h1>')
        self.server = server_module.make_server(0, self.static, self.root / 'data')
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.port = self.server.server_port
        self.item = dict(id='handoff-1', noteId='note-1', title='Build the thing', body='Line one\nLine two', agent='codex', createdAt='2026-11-16T08:00:00Z')

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.temporary.cleanup()

    def request(self, method='GET', path='/api/health', value=None, headers=None, raw=None):
        body = raw if raw is not None else (None if value is None else json.dumps(value).encode())
        actual_headers = {'Content-Type': 'application/json', **(headers or {})}
        connection = http.client.HTTPConnection('127.0.0.1', self.port, timeout=5)
        connection.request(method, path, body, actual_headers)
        response = connection.getresponse()
        data = response.read()
        status = response.status
        response_headers = dict(response.getheaders())
        connection.close()
        try:
            data = json.loads(data)
        except ValueError:
            pass
        return status, data, response_headers

    def add(self, item=None, headers=None):
        return self.request('POST', '/api/inbox', self.item if item is None else item, headers)

    def test_health_and_static_app(self):
        self.assertEqual(self.request()[1]['service'], 'nerv-fieldnotes')
        status, data, _ = self.request(path='/')
        self.assertEqual(status, 200)
        self.assertIn(b'Fieldnotes test', data)

    def test_write_list_markdown_and_ack(self):
        status, result, _ = self.add()
        self.assertEqual(status, 201)
        self.assertFalse(result['duplicate'])
        self.assertEqual(result['item']['status'], 'pending')
        listing = self.request(path='/api/inbox')[1]
        self.assertEqual(len(listing['items']), 1)
        self.assertIn('Line one\nLine two', (Path(listing['inboxPath']) / 'handoff-1.md').read_text())
        self.assertEqual(self.request('POST', '/api/inbox/handoff-1/ack', {})[1]['item']['status'], 'acknowledged')
        self.assertEqual(self.request('POST', '/api/inbox/handoff-1/ack', {})[0], 200)

    def test_retry_does_not_duplicate(self):
        self.add()
        status, result, _ = self.add()
        self.assertEqual(status, 200)
        self.assertTrue(result['duplicate'])
        self.assertEqual(len(self.request(path='/api/inbox')[1]['items']), 1)

    def test_retry_with_changed_content_conflicts(self):
        self.add()
        self.assertEqual(self.add({**self.item, 'body': 'Different content'})[0], 409)
        self.assertEqual(self.request(path='/api/inbox')[1]['items'][0]['body'], self.item['body'])

    def test_persists_in_database(self):
        self.add()
        reopened = server_module.Inbox(self.root / 'data')
        self.assertEqual(reopened.items()[0]['id'], 'handoff-1')
        self.assertEqual((self.root / 'data/inbox.sqlite3').stat().st_mode & 0o777, 0o600)

    def test_reject_cross_origin_and_null_origin(self):
        for origin in ('https://evil.example', 'null', 'http://localhost:1', f'http://localhost:{self.port}'):
            self.assertEqual(self.add(headers={'Origin': origin})[0], 403)
        self.assertEqual(self.add(headers={'Origin': f'http://127.0.0.1:{self.port}'})[0], 201)

    def test_reject_rebinding_host(self):
        self.assertEqual(self.request(headers={'Host': 'evil.example'})[0], 403)
        self.assertEqual(self.add(headers={'Host': 'evil.example'})[0], 403)

    def test_reject_wrong_content_type(self):
        self.assertEqual(self.add(headers={'Content-Type': 'text/plain'})[0], 415)

    def test_reject_invalid_payloads(self):
        values = [[], None, {}, {**self.item, 'extra': True}, {**self.item, 'body': False}, {**self.item, 'agent': ''}, {**self.item, 'createdAt': 'yesterday'}, {**self.item, 'createdAt': '2026-11-16T08:00:00'}, {**self.item, 'title': 'a' * 501}, {**self.item, 'id': '../escape'}]
        for value in values:
            with self.subTest(value=str(value)[:80]):
                self.assertEqual(self.request('POST', '/api/inbox', raw=json.dumps(value).encode())[0], 400)
        self.assertEqual(self.request('POST', '/api/inbox', raw=b'{broken')[0], 400)

    def test_reject_oversized_payload(self):
        self.assertEqual(self.request('POST', '/api/inbox', raw=b'x' * (server_module.MAX_BODY + 1))[0], 413)

    def test_reject_unknown_api_and_missing_handoff(self):
        self.assertEqual(self.request(path='/api/nope')[0], 404)
        self.assertEqual(self.request('POST', '/api/nope', {})[0], 404)
        self.assertEqual(self.request('POST', '/api/inbox/missing/ack', {})[0], 404)

    def test_blocks_traversal_hidden_files_and_symlinks(self):
        (self.root / 'secret.txt').write_text('private')
        (self.static / 'escape.txt').symlink_to(self.root / 'secret.txt')
        (self.static / '.secret').write_text('private')
        for path in ('/../secret.txt', '/%2e%2e/secret.txt', '/.secret', '/escape.txt'):
            self.assertEqual(self.request(path=path)[0], 403)
            self.assertEqual(self.request('HEAD', path)[0], 403)

    def test_no_cors_and_no_directory_listing(self):
        (self.static / 'empty').mkdir()
        self.assertEqual(self.request(path='/empty/')[0], 404)
        self.assertNotIn('Access-Control-Allow-Origin', self.request()[2])

    def test_cli_read_send_and_ack(self):
        command = [sys.executable, str(ROOT / 'scripts/fieldnotes-agent.py'), '--url', f'http://127.0.0.1:{self.port}']
        sent = subprocess.run(command + ['send', '--stdin'], input=json.dumps(self.item), capture_output=True, text=True)
        self.assertEqual(sent.returncode, 0, sent.stderr)
        listing = subprocess.run(command + ['list', '--agent', 'codex'], capture_output=True, text=True)
        self.assertEqual(json.loads(listing.stdout)['items'][0]['id'], 'handoff-1')
        acked = subprocess.run(command + ['ack', 'handoff-1'], capture_output=True, text=True)
        self.assertEqual(acked.returncode, 0, acked.stderr)
        listing = subprocess.run(command + ['list'], capture_output=True, text=True)
        self.assertEqual(json.loads(listing.stdout)['items'], [])
        shown = subprocess.run(command + ['show', 'handoff-1'], capture_output=True, text=True)
        self.assertEqual(json.loads(shown.stdout)['status'], 'acknowledged')

    def test_cli_rejects_external_origin(self):
        result = subprocess.run([sys.executable, str(ROOT / 'scripts/fieldnotes-agent.py'), '--url', 'https://evil.example', 'list'], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn('must be an http://localhost', result.stderr)


class InstallerTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which('systemd-analyze') and shutil.which('systemctl'), 'systemd tools are unavailable')
    def test_generated_user_unit_is_valid(self):
        with tempfile.TemporaryDirectory(prefix='fieldnotes-install-test-') as temporary:
            home = Path(temporary)
            env = {**os.environ, 'HOME': str(home), 'XDG_CONFIG_HOME': str(home / 'config'), 'XDG_DATA_HOME': str(home / 'data')}
            installed = subprocess.run(['bash', str(ROOT / 'scripts/install-local.sh'), '--no-start'], env=env, capture_output=True, text=True)
            self.assertEqual(installed.returncode, 0, installed.stderr)
            unit = home / 'config/systemd/user/nerv-fieldnotes.service'
            checked = subprocess.run(['systemd-analyze', '--user', 'verify', str(unit)], capture_output=True, text=True)
            self.assertEqual(checked.returncode, 0, checked.stderr)
            for name in ('nerv-fieldnotes', 'fieldnotes-agent'):
                launcher = home / '.local/bin' / name
                self.assertTrue(os.access(launcher, os.X_OK))
                syntax = subprocess.run(['bash', '-n', str(launcher)], capture_output=True, text=True)
                self.assertEqual(syntax.returncode, 0, syntax.stderr)
            self.assertIn('Name=NERV Fieldnotes', (home / 'data/applications/nerv-fieldnotes.desktop').read_text())


if __name__ == '__main__':
    unittest.main()
