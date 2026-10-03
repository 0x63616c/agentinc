instance = read_json('.local/dev/instance.json')

# A crate's own directory plus every workspace crate it depends on, so editing any of them rebuilds it.
packages = {p['name']: p for p in decode_json(local('cargo metadata --no-deps --format-version=1', quiet=True))['packages']}

def watch(name):
    seen = []
    todo = [name]
    for _ in range(len(packages)):
        if not todo:
            break
        current = todo.pop()
        seen.append(current)
        for dep in packages[current]['dependencies']:
            if dep.get('path') and dep['name'] in packages and dep['name'] not in seen and dep['name'] not in todo:
                todo.append(dep['name'])
    return ['Cargo.toml', 'Cargo.lock'] + [packages[n]['manifest_path'].rsplit('/', 1)[0] for n in seen]

docker_compose('dev/compose.yaml', env_file='.local/dev/compose.env', project_name=instance['id'], wait=True)
dc_resource('temporal', resource_deps=['postgres'])
dc_resource('temporal-ui', resource_deps=['temporal'])
local_resource(
    'aincd',
    serve_cmd='cargo xtask serve',
    deps=watch('ainc-xtask'),
    resource_deps=['postgres', 'temporal'],
    readiness_probe=probe(exec=exec_action(command=['sh', '-c', 'test -s .local/dev/api-url && curl -fsS "$(cat .local/dev/api-url)/health/ready" >/dev/null'])),
)
local_resource(
    'app',
    cmd='crates/ainc-mac/scripts/bundle.sh',
    serve_cmd='exec "crates/ainc-mac/dist/AgentInc Dev.app/Contents/MacOS/AgentInc"',
    serve_env={
        'AINC_DISCOVERY_FILE': os.getcwd() + '/.local/dev/api-url',
        'AINC_SESSION_PATH': os.getcwd() + '/.local/dev/session.json',
    },
    deps=watch('ainc-mac'),
    ignore=['crates/ainc-mac/dist', 'crates/ainc-mac/ghostty-bridge/.build'],
    resource_deps=['aincd'],
)
