#!/usr/bin/env python3
"""Exercise the deployed nginx config with synthetic OAuth callback secrets."""
import json
import subprocess
import time
import urllib.error
import urllib.request
import uuid
import sys

image = sys.argv[1]
name = 'diraigent-log-check-' + uuid.uuid4().hex[:10]
port = '44200'
marker = 'PRIVATE_CALLBACK_CANARY'
try:
    subprocess.run(['docker', 'run', '--rm', '-d', '--name', name,
                    '--add-host', 'diraigent-api:127.0.0.1',
                    '-p', f'127.0.0.1:{port}:80', image], check=True, capture_output=True)
    url = f'http://127.0.0.1:{port}/auth/callback?code={marker}&state={marker}'
    request = urllib.request.Request(url, headers={'Referer': f'https://example.test/?token={marker}'})
    for attempt in range(50):
        try:
            with urllib.request.urlopen(request, timeout=2) as response:
                assert response.status == 200
            break
        except (urllib.error.URLError, ConnectionError):
            if attempt == 49:
                raise RuntimeError('nginx did not start') from None
            time.sleep(0.1)
    # Verify failure paths without touching any production data or container.
    subprocess.run(['docker', 'exec', name, 'rm', '/usr/share/nginx/html/index.html'], check=True, capture_output=True)
    try:
        urllib.request.urlopen(request, timeout=2)
        raise AssertionError('missing callback page should return 404')
    except urllib.error.HTTPError as error:
        assert error.code == 404
    logs = subprocess.run(['docker', 'logs', name], check=True, capture_output=True, text=True)
    output = logs.stdout + logs.stderr
    assert marker not in output, 'synthetic callback/referrer secret appeared in nginx logs'
    records = [json.loads(line) for line in logs.stdout.splitlines() if line.startswith('{')]
    callbacks = [record for record in records if record.get('uri') == '/auth/callback']
    assert any(record['status'] == 200 for record in callbacks)
    assert any(record['status'] == 404 for record in callbacks)
    assert all('http_referer' not in record for record in callbacks)
    print('nginx callback privacy verified for successful and missing-page responses')
finally:
    subprocess.run(['docker', 'stop', name], capture_output=True)
