#!/usr/bin/env python3
"""Exercise onboarding with fake Docker/curl; no providers or deployments contacted."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
AGENT = '11111111-2222-4333-8444-555555555555'
KEY = 'dak_onboarding_test_not_a_real_key'

FAKE = '''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
args = sys.argv[1:]
state = Path(os.environ['FAKE_STATE'])
with (state/'calls').open('a') as f: f.write(json.dumps([Path(sys.argv[0]).name,args])+'\\n')
if Path(sys.argv[0]).name == 'docker':
    if 'auth' in args and 'list' in args:
        print(json.dumps([] if os.environ.get('NO_PROVIDER') else [{'id':'openai','connections':[{'type':'env','name':'OPENAI_API_KEY'}]}]))
    sys.exit(0)
if Path(sys.argv[0]).name == 'security':
    sys.exit('Claude Keychain must never be accessed')
if '/health/ready' in args[-1]: sys.exit(0)
out = Path(args[args.index('--output')+1])
method = args[args.index('--request')+1]
if os.environ.get('API_FAIL'):
    out.write_text(json.dumps({'error':'sensitive error body '+os.environ['TEST_KEY']}))
    print('403',end='');sys.exit(0)
if method == 'GET' and args[-1].endswith('/tenants'):
    out.write_text('[]');print('200',end='');sys.exit(0)
if method == 'POST' and args[-1].endswith('/tenants'):
    out.write_text(json.dumps({'id':os.environ['TEST_AGENT']}));print('200',end='');sys.exit(0)
if method == 'POST':
    headers=Path(args[args.index('--header')+1][1:]).read_text()
    (state/'registration_headers').write_text(headers)
    payload=json.loads(Path(args[args.index('--data-binary')+1][1:]).read_text())
    assert payload['metadata']['provider']=='opencode'
    assert 'kind' not in payload
    out.write_text(json.dumps({'id':os.environ['TEST_AGENT'],'api_key':os.environ['TEST_KEY']}))
else:
    out.write_text(json.dumps({'id':os.environ['TEST_AGENT']}))
print('200',end='')
'''

class Startup(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='diraigent-onboard-test-')
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)
        self.bin = self.path/'bin'; self.bin.mkdir()
        for name in ('docker','curl','security'):
            script=self.bin/name;script.write_text(FAKE);script.chmod(0o755)
        self.start=self.path/'start.sh';shutil.copyfile(ROOT/'startup/start.sh',self.start)
        self.envfile=self.path/'.env'
        self.envfile.write_text('DEV_USER_ID='+AGENT+'\nOPENCODE_MODEL=openai/test-model\nOPENAI_API_KEY=provider_test_not_real\n# keep comment\nMAX_WORKERS=1\n')
        self.env={k:v for k,v in os.environ.items() if k not in ('AGENT_ID','DIRAIGENT_API_TOKEN','STARTUP_API_TOKEN','OPENCODE_MODEL','DEV_USER_ID')}
        self.env.update(PATH=str(self.bin)+os.pathsep+self.env['PATH'],FAKE_STATE=str(self.path),TEST_AGENT=AGENT,TEST_KEY=KEY)

    def run_start(self,*args,**env):
        result=subprocess.run(['bash',str(self.start),*args],env={**self.env,**env},text=True,capture_output=True)
        self.assertNotIn(KEY,result.stdout+result.stderr)
        return result

    def calls(self):
        return [json.loads(line) for line in (self.path/'calls').read_text().splitlines()]

    def test_registration_persists_key_and_restart_reuses_identity(self):
        first=self.run_start();self.assertEqual(first.returncode,0,first.stderr)
        original=self.envfile.read_text()
        self.assertIn('AGENT_ID='+AGENT,original);self.assertIn('DIRAIGENT_API_TOKEN='+KEY,original)
        self.assertIn('# keep comment',original);self.assertIn('MAX_WORKERS=1',original)
        self.assertEqual(self.envfile.stat().st_mode & 0o777,0o600)
        again=self.run_start();self.assertEqual(again.returncode,0,again.stderr)
        self.assertEqual(self.envfile.read_text(),original)
        posts=[c for c in self.calls() if c[0]=='curl' and 'POST' in c[1] and c[1][-1].endswith('/agents')]
        self.assertEqual(len(posts),1)
        self.assertFalse(any(c[0]=='security' for c in self.calls()))
        self.assertFalse(any(KEY in str(c) for c in self.calls()))

    def test_unconnected_provider_never_starts_api_or_worker(self):
        result=self.run_start(NO_PROVIDER='1');self.assertNotEqual(result.returncode,0)
        self.assertIn('--login',result.stderr)
        self.assertFalse(any('up' in c[1] for c in self.calls()))

    def test_missing_model_fails_without_automatic_free_model(self):
        self.envfile.write_text('DEV_USER_ID='+AGENT+'\n')
        result=self.run_start();self.assertNotEqual(result.returncode,0)
        self.assertIn('OPENCODE_MODEL',result.stderr)
        self.assertFalse(any('up' in c[1] for c in self.calls()))

    def test_login_and_model_listing_work_without_claude_or_model(self):
        self.envfile.write_text('DEV_USER_ID='+AGENT+'\n')
        for flag in ('--login','--models'):
            result=self.run_start(flag);self.assertEqual(result.returncode,0,result.stderr)
        self.assertFalse(any(c[0]=='curl' for c in self.calls()))
        self.assertTrue(any('login' in c[1] and '--standalone' in c[1] for c in self.calls()))

    def test_existing_identity_without_key_is_not_replaced(self):
        self.envfile.write_text(self.envfile.read_text()+'AGENT_ID='+AGENT+'\n')
        before=self.envfile.read_text();result=self.run_start()
        self.assertNotEqual(result.returncode,0)
        self.assertEqual(self.envfile.read_text(),before)
        self.assertFalse(any(c[0]=='curl' and 'POST' in c[1] for c in self.calls()))

    def test_api_failure_keeps_identity_and_hides_response(self):
        self.envfile.write_text(self.envfile.read_text()+'AGENT_ID='+AGENT+'\nDIRAIGENT_API_TOKEN='+KEY+'\n')
        before=self.envfile.read_text();result=self.run_start(API_FAIL='1')
        self.assertNotEqual(result.returncode,0);self.assertIn('HTTP 403',result.stderr)
        self.assertEqual(self.envfile.read_text(),before)
        self.assertNotIn('sensitive error body',result.stderr)

    def test_build_uses_checkout_before_provider_login(self):
        startup=self.path/'startup';startup.mkdir()
        shutil.copyfile(self.start,startup/'start.sh');shutil.copyfile(self.envfile,startup/'.env')
        container=self.path/'apps/orchestra';container.mkdir(parents=True)
        (container/'Containerfile').touch()
        self.start=startup/'start.sh'
        result=self.run_start('--build','--login');self.assertEqual(result.returncode,0,result.stderr)
        builds=[c for c in self.calls() if 'build' in c[1]]
        self.assertEqual(builds[0][1],['compose','build','api','web','orchestra'])
        self.assertFalse(any(c[0]=='curl' for c in self.calls()))

    def test_symlink_env_is_rejected_before_registration(self):
        target=self.path/'original-env';self.envfile.rename(target)
        self.envfile.symlink_to(target)
        result=self.run_start();self.assertNotEqual(result.returncode,0)
        self.assertNotIn('DIRAIGENT_API_TOKEN=',target.read_text())

    def test_native_onboard_preserves_provider_settings_and_private_permissions(self):
        native=self.path/'onboard.sh';shutil.copyfile(ROOT/'apps/orchestra/onboard.sh',native)
        curl=self.bin/'curl'
        curl.write_text("#!/usr/bin/env python3\nimport sys\nif sys.argv[-1].endswith('/projects') or '/projects' in sys.argv: print('[]')\n")
        curl.chmod(0o755)
        before=self.envfile.read_text()
        result=subprocess.run(['bash',str(native),'--api','http://example.test/v1','--key',KEY,'--agent-id',AGENT],env=self.env,text=True,capture_output=True)
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertNotIn(KEY,result.stdout+result.stderr)
        self.assertIn(before,self.envfile.read_text())
        self.assertIn('OPENCODE_MODEL=openai/test-model',self.envfile.read_text())
        self.assertEqual(self.envfile.stat().st_mode & 0o777,0o600)

    def test_oidc_registration_uses_user_token_then_removes_it(self):
        self.envfile.write_text('OPENCODE_MODEL=openai/test-model\nSTARTUP_API_TOKEN=bootstrap_test_not_real\n')
        result=self.run_start();self.assertEqual(result.returncode,0,result.stderr)
        self.assertIn('Authorization: Bearer bootstrap_test_not_real',(self.path/'registration_headers').read_text())
        self.assertNotIn('STARTUP_API_TOKEN=',self.envfile.read_text())
        self.assertIn('DIRAIGENT_API_TOKEN=',self.envfile.read_text())

if __name__ == '__main__': unittest.main()
