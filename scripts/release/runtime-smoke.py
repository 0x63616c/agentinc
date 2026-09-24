#!/usr/bin/env python3
"""Fresh isolated profile: boot without external services, mutate, drain, restart.
State gates only; no fixed test sleeps. Own child and profile exclusively.
"""
import argparse
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import urllib.request
import uuid

parser=argparse.ArgumentParser()
parser.add_argument('bundle', type=Path)
parser.add_argument('profile', type=Path)
parser.add_argument('--workspace-tests',action='store_true')
parser.add_argument('--pilot',action='store_true')
parser.add_argument('--blocked-signals',action='store_true',help='regress inherited blocked/ignored SIGCHLD at daemon startup')
args=parser.parse_args()
version=json.loads((args.bundle/'Contents/Resources/release.json').read_text())['version']
root=args.profile.resolve()
root.mkdir(parents=True,exist_ok=False)
env=dict(os.environ,AINC_DISCOVERY_FILE=str(root/'api-url'),AINC_LEGACY_DIR=str(root/'legacy'),AGENTINC_CODEX_HOME=str(root/'codex'),RUST_LOG='info')
for name in ['DATABASE_URL','AINC_RUNTIME_CONFIG','AINC_DATABASE_URL']:
 env.pop(name,None)

def request(path,body=None):
 token=(root/'owner-token').read_text().strip()
 url=(root/'api-url').read_text().strip()+path
 headers={'Authorization':'Bearer '+token,'Agent-Inc-Client':f'mac/{version} (api 1)','Content-Type':'application/json'}
 req=urllib.request.Request(url,data=None if body is None else json.dumps(body).encode(),headers=headers,method='GET' if body is None else 'POST')
 with urllib.request.urlopen(req,timeout=10) as response:
  data=response.read()
  return json.loads(data) if data else None

def inherited_signals():
 signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGCHLD})
 signal.signal(signal.SIGCHLD, signal.SIG_IGN)

def assert_reaped(daemon):
 # Readiness requires a successful health-check child. It must have been reaped.
 rows=subprocess.check_output(['ps','-axo','pid=,ppid=,stat=,command='],text=True).splitlines()
 entries=[line.strip().split(None,3) for line in rows]
 children={int(row[0]) for row in entries if int(row[1])==daemon.pid}
 descendants=[row for row in entries if int(row[1]) in children]
 assert any('--local-runtime' in row[3] for row in entries if int(row[0]) in children)
 assert not any('Z' in row[2] for row in descendants), 'runtime helper left a zombie health-check child'

def start():
 child=subprocess.Popen([str(args.bundle.resolve()/'Contents/MacOS/aincd')],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,preexec_fn=inherited_signals if args.blocked_signals else None)
 selector=selectors.DefaultSelector();selector.register(child.stdout,selectors.EVENT_READ)
 while selector.select(timeout=90):
  line=child.stdout.readline()
  if not line:
   child.wait();raise RuntimeError('daemon exited: '+str(child.returncode))
  with (root/'daemon.log').open('a') as log:log.write(line)
  if 'AgentInc daemon ready' in line:
   selector.close()
   if args.blocked_signals: assert_reaped(child)
   return child
 child.terminate();child.wait();raise RuntimeError('daemon readiness deadline')

child=None
try:
 child=start()
 assert request('/health/ready')['status']=='ready'
 request('/v1/commands',{'operation_id':str(uuid.uuid4()),'command':{'kind':'create_conversation'}})
 first=request('/v1/state')
 print('Fresh bundled runtime ready'+ (' with inherited blocked/ignored SIGCHLD; health-check child reaped' if args.blocked_signals else ''))
 if args.workspace_tests:
  pid=(root/'runtime/postgres/postmaster.pid').read_text().splitlines()
  password=(root/'runtime/postgres-password').read_text()
  testenv=dict(os.environ,DATABASE_URL=f'postgres://agentinc:{password}@127.0.0.1:{pid[3]}/postgres',CARGO_INCREMENTAL='0')
  with (root/'workspace-tests.log').open('w') as log:
   subprocess.run(['cargo','test','--locked','--workspace'],env=testenv,stdout=log,stderr=subprocess.STDOUT,check=True)
 if args.pilot:
  pilotenv=dict(os.environ,AINC_DISCOVERY_FILE=str(root/'api-url'),CARGO_INCREMENTAL='0')
  with (root/'pilot.log').open('w') as log:
   subprocess.run(['cargo','build','--locked','-p','agentinc-os','-p','gpui-pilot-cli','--features','agentinc-os/automation'],env=pilotenv,stdout=log,stderr=subprocess.STDOUT,check=True)
   subprocess.run(['cargo','test','--locked','-p','agentinc-os','--features','automation','--test','pilot_acceptance','--','--nocapture'],env=pilotenv,stdout=log,stderr=subprocess.STDOUT,check=True)
  first=request('/v1/state')
 request('/internal/drain',{})
 assert child.wait(timeout=120)==0
 child=None
 assert not (root/'api-url').exists()
 child=start()
 assert request('/v1/state')==first
 print('Restart retained state; runtime recovered on fresh ports')
 request('/internal/drain',{})
 assert child.wait(timeout=120)==0
 child=None
 print('Drain completed and removed discovery')
 child=start()
 child.kill()
 child.wait(timeout=30)
 child=None
 child=start()
 assert request('/v1/state')==first
 request('/internal/drain',{})
 assert child.wait(timeout=120)==0
 child=None
 print('Hard-killed daemon recovered without orphaned-runtime interference')
finally:
 if child is not None and child.poll() is None:
  child.terminate();child.wait(timeout=120)
