"""Run the real install script in disposable containers, with package commands stubbed."""
import base64
import json
import pathlib
import subprocess

script = pathlib.Path('infra/install.sh').read_text()
bundle = {
    'binding': 'fixture',
    'material': {'protocol': 'amnezia_wg', 'server_config': 'secret-fixture', 'profile': {}},
    'awg_dockerfile': 'fixture', 'awg_entrypoint': 'fixture', 'manifest': '{}',
}
encoded = base64.b64encode(json.dumps(bundle).encode()).decode()
for payload, stage, status in [(encoded, 'packages', 42), ('invalid', 'state', 1)]:
    # No package or service changes: apt-get fails deliberately before installing anything.
    body = f"MS_BUNDLE='{payload}'\napt-get() {{ return 42; }}\n" + script
    result = subprocess.run(
        ['docker', 'run', '--rm', '-i', '--network=none', 'myserv-native-check', 'bash', '-se'],
        input=body, text=True, capture_output=True, check=False,
    )
    assert result.returncode == status, result
    assert f'MYSERV_ERROR:install-{stage}\n' in result.stdout, result.stdout
    assert 'MYSERV_DETAIL:' in result.stdout, result.stdout
    assert 'secret-fixture' not in result.stdout
    assert encoded not in result.stdout
print('Installer failures: stage and numeric location reported, secrets stay off stdout: PASS')
