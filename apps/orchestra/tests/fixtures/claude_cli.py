import sys, os, json, pathlib, stat, urllib.request, urllib.error, time

mode = os.environ['FAKE_MODE']
if '--version' in sys.argv:
    print('2.1.86 (Claude Code)' if mode == 'old_version' else '2.1.246 (Claude Code)')
    sys.exit(0)
if '--help' in sys.argv:
    print('old CLI' if mode == 'unsupported' else '--mcp-config --strict-mcp-config --permission-mode dontAsk --setting-sources --no-chrome')
    sys.exit(0)
args = sys.argv[1:]
capture = {'args': args, 'pid': os.getpid()}
if '--mcp-config' in args:
    path = pathlib.Path(args[args.index('--mcp-config') + 1])
    capture.update(runtime=str(path.parent), file_mode=stat.S_IMODE(path.stat().st_mode), dir_mode=stat.S_IMODE(path.parent.stat().st_mode))
    config = json.loads(path.read_text())
    assert 'upstream-secret' not in path.read_text()
    if mode == 'empty':
        assert config == {'mcpServers': {}}
        pathlib.Path(os.environ['FAKE_CAPTURE']).write_text(json.dumps(capture))
        print('{"type":"result","result":"ok","is_error":false}')
        sys.exit(0)
    assert list(config['mcpServers']) == ['orchestra_0']
    server = config['mcpServers']['orchestra_0']
    assert set(server) == {'type', 'url', 'headers'}
    assert server['headers']['Authorization'] not in (path.parent / 'run.sh').read_text()
    capture['endpoint'] = server['url']
    def rpc(method, params=None, auth=True):
        data = json.dumps({'jsonrpc':'2.0', 'id':1, 'method':method, 'params':params or {}}).encode()
        headers = server['headers'] if auth else {}
        request = urllib.request.Request(server['url'], data=data, headers={**headers, 'Content-Type':'application/json'})
        try:
            return json.load(urllib.request.urlopen(request))
        except urllib.error.HTTPError as e:
            return e.code
    assert rpc('initialize')['result']['capabilities'] == {'tools':{}}
    capture['catalog'] = rpc('tools/list')
    capture['allowed'] = rpc('tools/call', {'name':'read','arguments':{}})
    capture['denied'] = rpc('tools/call', {'name':'unapproved','arguments':{}})
    capture['unauthenticated'] = rpc('tools/list', auth=False)
pathlib.Path(os.environ['FAKE_CAPTURE']).write_text(json.dumps(capture))
if mode == 'sleep':
    time.sleep(60)
if mode == 'failure':
    sys.exit(7)
print(json.dumps({'type':'result','result':'ok','is_error':False}, separators=(',', ':')))
