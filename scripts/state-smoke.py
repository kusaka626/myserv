"""Exercises the actual server state initializer in disposable Docker storage."""
import base64,json,pathlib,subprocess,uuid
script=pathlib.Path('infra/install.sh').read_text().split("python3 - <<'PY'\n",1)[1].split('\nPY\n',1)[0]
volume='myserv-state-test-'+uuid.uuid4().hex
image='myserv-native-check'
def call(body):
 payload={'binding':'fixture-binding','material':{'protocol':'amnezia_wg','server_config':'original','profile':{'content':'original'}},'awg_dockerfile':'test','awg_entrypoint':'test','manifest':'{}'}
 payload.update(body)
 return subprocess.run(['docker','run','--rm','-i','-v',volume+':/var/lib/myserv','-e','MS_BUNDLE='+base64.b64encode(json.dumps(payload).encode()).decode(),image,'python3','-'],input=script,text=True,capture_output=True)
try:
 subprocess.run(['docker','volume','create',volume],check=True,capture_output=True)
 assert call({}).returncode==0
 assert call({'material':{'protocol':'amnezia_wg','server_config':'replacement','profile':{'content':'replacement'}}}).returncode==0
 verify="import json; s=json.load(open('/var/lib/myserv/state.json')); assert s['material']['server_config']=='original'; assert s['material']['profile']['content']=='original'"
 subprocess.run(['docker','run','--rm','-v',volume+':/var/lib/myserv',image,'python3','-c',verify],check=True,capture_output=True)
 assert call({'binding':'different-server'}).returncode!=0
 assert call({'material':{'protocol':'vless_reality','server_config':'replacement','profile':{}}}).returncode!=0
 print('Server state: retry preserves keys; wrong binding and protocol migration rejected: PASS')
finally:
 subprocess.run(['docker','volume','rm',volume],capture_output=True,check=False)
