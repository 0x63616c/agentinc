#!/usr/bin/env python3
"""Fresh isolated profile: boot without external services, mutate, drain, restart.
State gates only; no fixed test sleeps. Own child and profile exclusively.
"""
import argparse
import json
import os
from pathlib import Path
import selectors
import subprocess
import urllib.request

parser=argparse.ArgumentParser()
parser.add_argument('bundle', type=Path)
parser.add_argument('profile', type=Path)
parser.add_argument('--workspace-tests',action='store_true')
args=parser.parse_args()
root=args.profile.resolve()
root.mkdir(parents=True,exist_ok=False)
env=dict(os.environ,AINC_DISCOVERY_FILE=str(root/'api-url'),AINC_LEGACY_DIR=str(root/'legacy'),AGENTINC_CODEX_HOME=str(root/'codex'),RUST_LOG='info')
for name in ['DATABASE_URL','AINC_RUNTIME_CONFIG','AINC_DATABASE_URL']:
 env.pop(name,None)

def request(path,body=None):
 token=(root/'owner-token').read_text().strip()
 url=(root/'api-url').read_text().strip()+path
 headers={'Authorization':'Bearer '+token,'Agent-Inc-Client':'mac/0.1.0 (api 1)','Content-Type':'application/json'}
 req=urllib.request.Request(url,data=None if body is None else json.dumps(body).encode(),headers=headers,method='GET' if body is None else 'POST')
 with urllib.request.urlopen(req,timeout=10) as response:
  data=response.read()
  return json.loads(data) if data else None

def start():
 child=subprocess.Popen([str(args.bundle.resolve()/'Contents/MacOS/aincd')],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 selector=selectors.DefaultSelector();selector.register(child.stdout,selectors.EVENT_READ)
 while selector.select(timeout=90):
  line=child.stdout.readline()
  if not line:
   child.wait();raise RuntimeError('daemon exited: '+str(child.returncode))
  with (root/'daemon.log').open('a') as log:log.write(line)
  if 'AgentInc daemon ready' in line:
   selector.close();return child
 child.terminate();child.wait();raise RuntimeError('daemon readiness deadline')

child=None
try:
 child=start()
 assert request('/health/ready')['status']=='ready'
 first=request('/v1/state')
 print('Fresh bundled runtime ready')
 if args.workspace_tests:
  pid=(root/'runtime/postgres/postmaster.pid').read_text().splitlines()
  password=(root/'runtime/postgres-password').read_text()
  testenv=dict(os.environ,DATABASE_URL=f'postgres://agentinc:{password}@127.0.0.1:{pid[3]}/postgres',CARGO_INCREMENTAL='0')
  with (root/'workspace-tests.log').open('w') as log:
   subprocess.run(['cargo','test','--locked','--workspace'],env=testenv,stdout=log,stderr=subprocess.STDOUT,check=True)
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
finally:
 if child is not None and child.poll() is None:
  child.terminate();child.wait(timeout=120)
