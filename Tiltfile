instance = read_json('.local/dev/instance.json')
docker_compose('dev/compose.yaml', env_file='.local/dev/compose.env', project_name=instance['id'], wait=True)
dc_resource('temporal', resource_deps=['postgres'])
dc_resource('temporal-ui', resource_deps=['temporal'])
local_resource(
    'aincd',
    serve_cmd='cargo xtask serve',
    deps=['crates/ainc-xtask/src/main.rs', 'crates/ainc-daemon/src'],
    resource_deps=['postgres', 'temporal'],
    readiness_probe=probe(exec=exec_action(command=['sh', '-c', 'test -s .local/dev/api-url && curl -fsS "$(cat .local/dev/api-url)/health/ready" >/dev/null'])),
)
