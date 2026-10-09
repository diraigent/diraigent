#!/usr/bin/env python3
"""Verify API recreation and readiness using an isolated Docker network."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

image = sys.argv[1]
prefix = 'diraigent-routing-' + uuid.uuid4().hex[:10]
network = prefix + '-net'
containers = []


def docker(*args):
    result = subprocess.run(['docker', *args], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    return result.stdout.strip()


def backend(config, name):
    containers.append(name)
    docker('run', '--rm', '-d', '--name', name, '--network', network,
           '--network-alias', 'diraigent-api', '-v', f'{config}:/tmp/backend.conf:ro',
           '--entrypoint', 'nginx', image, '-c', '/tmp/backend.conf',
           '-g', 'daemon off;')
    return docker('inspect', '-f', '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}', name)


def response(url, expected):
    for attempt in range(80):
        try:
            with urllib.request.urlopen(url, timeout=2) as result:
                body = json.load(result)
            if body['backend'] == expected:
                return body
        except (urllib.error.URLError, ConnectionError, ValueError):
            pass
        time.sleep(0.2)
    raise AssertionError(f'API routing did not reach backend {expected}')


try:
    docker('network', 'create', network)
    # Linux Docker permits static addresses only with an explicitly configured
    # subnet. Reuse Docker's allocated subnet instead of hard-coding a host range.
    subnet = json.loads(docker('network', 'inspect', network))[0]['IPAM']['Config'][0]['Subnet']
    docker('network', 'rm', network)
    docker('network', 'create', '--subnet', subnet, network)
    with tempfile.TemporaryDirectory() as directory:
        configs = []
        for identity in ['first', 'replacement']:
            config = Path(directory) / f'{identity}.conf'
            config.write_text('events {}\nhttp { access_log off; server { listen 3000; '
                              'location / { default_type application/json; return 200 '
                              f"'{{\"backend\":\"{identity}\",\"uri\":\"$request_uri\"}}'; "
                              '} } }\n')
            configs.append(config)
        first = prefix + '-first'
        old_ip = backend(configs[0], first)
        web = prefix + '-web'
        containers.append(web)
        docker('run', '--rm', '-d', '--name', web, '--network', network,
               '-p', '127.0.0.1::80', image)
        binding = json.loads(docker('inspect', '-f', '{{json .NetworkSettings.Ports}}', web))
        base = 'http://127.0.0.1:' + binding['80/tcp'][0]['HostPort']
        path = '/v1/example/tasks?limit=2&offset=1'
        assert response(base + path, 'first')['uri'] == path
        docker('stop', first)
        # Occupy the old address so the replacement must receive a different IP.
        holder = prefix + '-holder'
        containers.append(holder)
        docker('run', '--rm', '-d', '--name', holder, '--network', network,
               '--ip', old_ip, '--entrypoint', 'sleep', image, '120')
        new_ip = backend(configs[1], prefix + '-replacement')
        assert new_ip != old_ip
        for path in ['/v1', '/v1/example/tasks?limit=2&offset=1',
                     '/api/example?value=1', '/health/live', '/health/ready']:
            assert response(base + path, 'replacement')['uri'] == path
        docker('exec', web, 'wget', '--quiet', '--tries=1', '--spider',
               'http://127.0.0.1/health/ready')
        docker('stop', prefix + '-replacement')
        failed = subprocess.run(['docker', 'exec', web, 'wget', '--quiet',
                                 '--tries=1', '--spider', 'http://127.0.0.1/health/ready'],
                                capture_output=True)
        assert failed.returncode != 0, 'readiness must fail when the API is unavailable'
        print('API routing follows address changes, preserves URLs, and checks readiness')
finally:
    for name in reversed(containers):
        subprocess.run(['docker', 'stop', name], capture_output=True)
    subprocess.run(['docker', 'network', 'rm', network], capture_output=True)
