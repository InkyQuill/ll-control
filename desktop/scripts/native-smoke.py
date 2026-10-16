#!/usr/bin/env python3
"""Run only the native-smoke binary and MockBackend fixture under isolated Xvfb."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
import tomllib

repo = Path(__file__).resolve().parents[2]
root = Path(tempfile.mkdtemp(prefix="lconnect-native-"))
(root / "fixture-marker").write_text("MockBackend only\n")
# A valid asset from a prior session, intentionally absent from every theme/profile.
font_orphan = root / "settings/theme-fonts/99999999999999999999999999999999.font"
font_orphan.parent.mkdir(parents=True)
font_orphan.write_bytes((repo / "crates/lconnect-image/assets/DejaVuSans.ttf").read_bytes())
font_keys = {"offline": "d" * 32, "profile": "e" * 32}
font_bytes = (repo / "crates/lconnect-image/assets/DejaVuSans.ttf").read_bytes()
for key in font_keys.values():
    (font_orphan.parent / (key + ".font")).write_bytes(font_bytes)
# Prior-lifetime assets: profile-only, offline-device-only, orphan.
image_keys = {"offline": "a" * 32, "profile": "c" * 32, "orphan": "b" * 32}
image_root = root / "settings/theme-assets"
image_root.mkdir(parents=True)
for key in image_keys.values():
    (image_root / (key + ".png")).write_bytes((repo / "desktop/src-tauri/icons/icon.png").read_bytes())
def retained_image_theme(key):
    font_key = font_keys[next(kind for kind in font_keys if image_keys[kind] == key)]
    return {"version": 1, "name": "Retained image", "background": [0, 0, 0], "layers": [
        {"kind": "image", "asset": key, "x": 0, "y": 0, "width": 480, "height": 480, "fit": "contain", "opacity": 255},
        {"kind": "text", "text": "Retained font", "style": {"font_asset": font_key, "x": 240, "y": 240, "size": 32, "color": [255, 255, 255], "align": "center"}}]}
(root / "settings/profiles.json").write_text(json.dumps({"version": 1, "revision": 0, "profiles": [
    {"slot": 6, "name": "Offline retained profile", "content": {"kind": "screen", "value": retained_image_theme(image_keys["profile"])}}]}))
(root / "settings/devices").mkdir()
(root / "settings/devices/offline.toml").write_text('[theme]\nversion = 1\nname = "Offline fixture"\nbackground = [0, 0, 0]\n[[theme.layers]]\nkind = "image"\nasset = "' + image_keys["offline"] + '"\nx = 0\ny = 0\nwidth = 480\nheight = 480\nfit = "contain"\nopacity = 255\n')
with (root / "settings/devices/offline.toml").open("a") as settings:
    settings.write('[[theme.layers]]\nkind = "text"\ntext = "Offline font"\n[theme.layers.style]\nfont_asset = "' + font_keys["offline"] + '"\nx = 240\ny = 240\nsize = 32\ncolor = [255, 255, 255]\nalign = "center"\n')
assert tomllib.loads((root / "settings/devices/offline.toml").read_text())["theme"]["layers"][0]["asset"] == image_keys["offline"]
env = dict(os.environ, LCONNECT_SMOKE_ROOT=str(root), XDG_RUNTIME_DIR=str(root),
           XDG_CONFIG_HOME=str(root / "config"), XDG_CACHE_HOME=str(root / "cache"),
           XDG_DATA_HOME=str(root / "data"), GDK_BACKEND="x11",
           WEBKIT_DISABLE_DMABUF_RENDERER="1")
env.pop("WAYLAND_DISPLAY", None)
# Only the isolated fixture process receives the preload; all outputs stay private.
unlink_shim = root / 'native-unlink-fault.so'
subprocess.run(['cc', '-shared', '-fPIC', '-Wall', '-Wextra', '-Werror',
                str(repo / 'desktop/scripts/native-unlink-fault.c'), '-o', str(unlink_shim)], check=True)
fixture_env = dict(env, LD_PRELOAD=str(unlink_shim))

fixture = None
logs = []
proxy = None
proxy_spec = importlib.util.spec_from_file_location("native_fault_proxy", repo / "desktop/scripts/native-fault-proxy.py")
proxy_module = importlib.util.module_from_spec(proxy_spec)
proxy_spec.loader.exec_module(proxy_module)



def start():
    global fixture
    (root / "stop").unlink(missing_ok=True)
    (root / "stats.json").unlink(missing_ok=True)
    (root / "lconnectd.sock").unlink(missing_ok=True)
    log = (root / f"daemon-{len(logs)}.log").open("w")
    logs.append(log)
    fixture = subprocess.Popen([repo / "target/debug/examples/desktop-mock"],
                               env=fixture_env, stdout=log, stderr=subprocess.STDOUT)
    for _ in range(100):
        if fixture.poll() is not None:
            raise RuntimeError("Mock fixture exited")
        if (root / "lconnectd.sock").exists() and (root / "stats.json").exists():
            return
        time.sleep(.1)
    raise TimeoutError("mock socket")


def stop():
    global fixture
    if fixture is not None:
        (root / "stop").touch()
        try:
            fixture.wait(timeout=10)
        except subprocess.TimeoutExpired:
            fixture.kill()
            fixture.wait()
            raise
        if fixture.returncode:
            raise RuntimeError(f"mock fixture exit {fixture.returncode}")
        fixture = None


def rpc(method, params):
    with socket.socket(socket.AF_UNIX) as conn:
        conn.settimeout(10)
        conn.connect(str(root / "lconnectd.sock"))
        stream = conn.makefile("rwb")
        stream.write((json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}) + "\n").encode())
        stream.flush()
        while True:
            response = json.loads(stream.readline())
            if response.get("id") == 1:
                if "error" in response:
                    raise RuntimeError(response["error"])
                return response["result"]


profile_cleanup_lock = threading.Lock()
def cleanup_profile_fault(request):
    # Serialize the orchestration reader with corruption; GUI still uses real IPC.
    with profile_cleanup_lock:
        path = root / 'settings/profiles.json'
        original = path.read_bytes()
        assets = {p: p.read_bytes() for folder in (image_root, font_orphan.parent) for p in folder.iterdir() if p.is_file()}
        try:
            path.write_bytes(b'{broken profile JSON')
            try:
                rpc(request['method'], request['params'])
            except RuntimeError as error:
                response = {'jsonrpc': '2.0', 'id': request['id'], 'error': error.args[0]}
            else:
                raise AssertionError('cleanup accepted corrupt profile JSON')
            assert path.read_bytes() == b'{broken profile JSON', 'cleanup overwrote invalid library'
            assert all(p.read_bytes() == data for p, data in assets.items()), 'cleanup changed assets with corrupt profiles'
            return response
        finally:
            path.write_bytes(original)

try:
    start()
    gui_runtime = root / "gui-runtime"
    gui_runtime.mkdir(mode=0o700)
    (gui_runtime / "fixture-marker").write_text("MockBackend proxy only\n")
    proxy = proxy_module.FaultProxy(gui_runtime / "lconnectd.sock", root / "lconnectd.sock", cleanup_profile_fault=cleanup_profile_fault)
    gui_env = dict(env, XDG_RUNTIME_DIR=str(gui_runtime), LCONNECT_SMOKE_ROOT=str(gui_runtime))
    with (root / "gui.log").open("w") as log:
        proc = subprocess.Popen(["xvfb-run", "-a", "dbus-run-session", "--",
                                 str(repo / "target/debug/lconnect-gui")],
                                env=gui_env, stdout=log, stderr=subprocess.STDOUT,
                                start_new_session=True)
        try:
            deadline = time.monotonic() + 180
            restarted_pages = set()
            sensor_faults = set()
            consumer_faults = set()
            transport_observations = {}
            cleanup_faults = set()
            partial_cleanup_faults = set()
            offline_settings = root / "settings/devices/offline.toml"
            offline_saved = offline_settings.read_bytes()
            cleanup_image_bytes = {key: (image_root / (key + ".png")).read_bytes() for key in image_keys.values()}
            dependency_faults = set()
            dependency_backups = {}
            restart_dependency_faults = set()
            restart_dependency_backups = {}
            while proc.poll() is None:
                if time.monotonic() >= deadline:
                    raise subprocess.TimeoutExpired(proc.args, 180)
                with profile_cleanup_lock:
                    library = rpc("list_profiles", {})
                for profile in library['profiles']:
                    marker = profile['name']
                    if not marker.startswith('Partial cleanup request:') or marker in partial_cleanup_faults:
                        continue
                    _, kind, phase = marker.split(':')
                    assert kind in ('image', 'font') and phase in ('seed', 'restore')
                    folder, suffix, data = (image_root, '.png', (repo / 'desktop/src-tauri/icons/icon.png').read_bytes()) if kind == 'image' else (font_orphan.parent, '.font', font_bytes)
                    failed, removed = folder / ('f' * 32 + suffix), folder / ('8' * 32 + suffix)
                    if phase == 'seed':
                        assert not failed.exists() and not removed.exists()
                        failed.write_bytes(data)
                        removed.write_bytes(data)
                        (root / 'fail-unlink').touch()
                    else:
                        assert failed.read_bytes() == data and not removed.exists(), 'partial cleanup did not preserve only failed orphan'
                        (root / 'fail-unlink').unlink()
                    current = rpc('list_profiles', {})
                    rpc('edit_profile', {'expected_revision': current['revision'], 'edit': {
                        'action': 'rename', 'group': 'screen', 'slot': 6,
                        'name': marker.replace('request', 'ready')}})
                    partial_cleanup_faults.add(marker)
                for profile in library['profiles']:
                    marker = profile['name']
                    if profile['content']['kind'] != 'screen' or marker not in {'Cleanup request:corrupt', 'Cleanup request:restore'} or marker in cleanup_faults:
                        continue
                    if marker.endswith('corrupt'):
                        offline_settings.write_bytes(b'broken = [')
                    else:
                        for key, saved_bytes in cleanup_image_bytes.items():
                            assert (image_root / (key + ".png")).read_bytes() == saved_bytes, "cleanup deleted or changed assets with corrupt settings"
                        offline_settings.write_bytes(offline_saved)
                    current = rpc('list_profiles', {})
                    rpc('edit_profile', {'expected_revision': current['revision'], 'edit': {
                        'action': 'rename', 'group': 'screen', 'slot': 6,
                        'name': marker.replace('request', 'ready')}})
                    cleanup_faults.add(marker)
                for profile in library['profiles']:
                    marker = profile['name']
                    if profile['content']['kind'] != 'screen' or marker not in {'Consumer request:missing', 'Consumer request:restore', 'Consumer request:hot', 'Consumer request:stale', 'Consumer request:cool', 'Consumer request:rgb-fail', 'Consumer request:rgb-restore', 'Consumer request:lcd-fail', 'Consumer request:lcd-restore', 'Consumer request:pwm-fail', 'Consumer request:pwm-restore'} or marker in consumer_faults:
                        continue
                    if marker.endswith(('lcd-fail', 'pwm-fail')):
                        kind = 'lcd' if marker.endswith('lcd-fail') else 'pwm'
                        transport_observations[kind] = {'before': json.loads((root / 'stats.json').read_text())}
                        if kind == 'lcd':
                            transport_observations[kind]['saved_themes'] = {
                                str(p.relative_to(root)): tomllib.loads(p.read_text()).get('theme')
                                for p in (root / 'settings/devices').glob('*.toml')}
                            assert any(t and t.get('name') == 'Native DOM applied' for t in transport_observations[kind]['saved_themes'].values())
                        (root / ('fail-lcd' if kind == 'lcd' else 'fail-fan-pwm')).touch()
                    elif marker.endswith(('lcd-restore', 'pwm-restore')):
                        kind = 'lcd' if marker.endswith('lcd-restore') else 'pwm'
                        observation = transport_observations[kind]
                        observation['during'] = json.loads((root / 'stats.json').read_text())
                        before, during = observation['before'], observation['during']
                        failure_key = 'lcd_failures' if kind == 'lcd' else 'fan_pwm_failures'
                        assert during[failure_key] > before[failure_key], kind + ' fault not exercised'
                        assert during['pump_rgb_writes'] > before['pump_rgb_writes'], 'RGB did not progress during ' + kind
                        if kind == 'lcd':
                            for path, theme in observation['saved_themes'].items():
                                assert tomllib.loads((root / path).read_text()).get('theme') == theme, 'LCD error lost persisted theme'
                            observation['saved_theme_preserved'] = True
                            assert during['fan_pwm_writes'] > before['fan_pwm_writes'], 'cooling stopped during LCD error'
                        else:
                            assert during['jpeg_starts'] >= before['jpeg_starts'] + 2, 'LCD stopped during PWM error'
                        (root / ('fail-lcd' if kind == 'lcd' else 'fail-fan-pwm')).unlink()
                    elif marker.endswith('rgb-fail'):
                        transport_observations['before'] = json.loads((root / 'stats.json').read_text())
                        (root / 'fail-pump-rgb').touch()
                    elif marker.endswith('rgb-restore'):
                        transport_observations['during'] = json.loads((root / 'stats.json').read_text())
                        before, during = transport_observations['before'], transport_observations['during']
                        assert during['pump_rgb_failures'] > before['pump_rgb_failures'], 'RGB fault was not exercised'
                        assert during['jpeg_starts'] >= before['jpeg_starts'] + 2, 'LCD stopped during RGB fault'
                        assert during['fan_pwm_writes'] > before['fan_pwm_writes'], 'cooling did not write during RGB fault'
                        (root / 'fail-pump-rgb').unlink()
                    elif marker.endswith('missing'):
                        (root / 'cpu-unavailable').touch()
                    elif marker.endswith('restore'):
                        (root / 'cpu-unavailable').unlink()
                    elif marker.endswith('hot'):
                        (root / 'cpu-hot').touch()
                    elif marker.endswith('stale'):
                        (root / 'freeze-sensors').touch()
                    else:
                        (root / 'cpu-hot').unlink()
                        (root / 'freeze-sensors').unlink()
                    current = rpc('list_profiles', {})
                    rpc('edit_profile', {'expected_revision': current['revision'], 'edit': {
                        'action': 'rename', 'group': 'screen', 'slot': 0,
                        'name': marker.replace('request', 'ready')}})
                    consumer_faults.add(marker)
                for profile in library['profiles']:
                    marker = profile['name']
                    if profile['content']['kind'] == 'screen' and marker in {'Sensor request:missing', 'Sensor request:restore'} and marker not in sensor_faults:
                        if marker.endswith('missing'):
                            (root / 'cpu-unavailable').touch()
                        else:
                            (root / 'cpu-unavailable').unlink()
                        rpc('edit_profile', {'expected_revision': library['revision'], 'edit': {
                            'action': 'rename', 'group': 'screen', 'slot': 0,
                            'name': marker.replace('request', 'ready')}})
                        sensor_faults.add(marker)
                for profile in library['profiles']:
                    marker = profile['name']
                    if profile['content']['kind'] != 'screen' or not marker.startswith('Dependency request:') or marker in dependency_faults:
                        continue
                    _, kind, mode = marker.split(':')
                    theme = profile['content']['value']
                    key = theme['layers'][3]['style']['font_asset'] if kind == 'font' else theme['layers'][0]['asset']
                    path = root / 'settings' / ('theme-fonts' if kind == 'font' else 'theme-assets') / (key + ('.font' if kind == 'font' else '.png'))
                    if kind not in dependency_backups:
                        dependency_backups[kind] = path.read_bytes()
                    if mode == 'missing':
                        path.unlink()
                    elif mode == 'corrupt':
                        path.write_bytes(b'corrupt fixture dependency')
                    elif mode.startswith('restore'):
                        path.write_bytes(dependency_backups[kind])
                    else:
                        raise AssertionError('unknown dependency fault')
                    rpc('edit_profile', {'expected_revision': library['revision'], 'edit': {
                        'action': 'rename', 'group': 'screen', 'slot': 0,
                        'name': marker.replace('request', 'ready')}})
                    dependency_faults.add(marker)
                for profile in library['profiles']:
                    marker = profile['name']
                    if profile['content']['kind'] != 'screen' or not marker.startswith('Restart dependency request:') or marker in restart_dependency_faults:
                        continue
                    _, kind, mode = marker.split(':')
                    theme = profile['content']['value']
                    key = theme['layers'][3]['style']['font_asset'] if kind == 'font' else theme['layers'][0]['asset']
                    path = root / 'settings' / ('theme-fonts' if kind == 'font' else 'theme-assets') / (key + ('.font' if kind == 'font' else '.png'))
                    stop()  # Flush before taking the saved-settings baseline.
                    settings = {p.name: tomllib.loads(p.read_text()) for p in (root / 'settings/devices').glob('*.toml')}
                    if mode == 'missing':
                        assert settings and any(v.get('theme') == theme for v in settings.values()), 'active theme was not durable before fault'
                        restart_dependency_backups[kind] = (path.read_bytes(), settings)
                        path.unlink()
                    elif mode == 'restore':
                        data, original_settings = restart_dependency_backups[kind]
                        assert settings == original_settings, 'failed replay rewrote saved configuration'
                        path.write_bytes(data)
                    else:
                        raise AssertionError('unknown restart dependency fault')
                    time.sleep(4)  # GUI must observe the disconnected state.
                    start()
                    for _ in range(100):
                        if rpc('list_devices', {}):
                            break
                        time.sleep(.1)
                    else:
                        raise AssertionError('device unavailable after dependency restart')
                    if mode == 'missing':
                        assert json.loads((root / 'stats.json').read_text())['jpeg_starts'] == 0, 'missing dependency emitted a JPEG'
                    current_library = rpc('list_profiles', {})
                    rpc('edit_profile', {'expected_revision': current_library['revision'], 'edit': {
                        'action': 'rename', 'group': 'screen', 'slot': 0,
                        'name': marker.replace('request', 'ready')}})
                    restart_dependency_faults.add(marker)
                markers = {"Native reconnect ready", "Native reconnect cooling",
                           "Native reconnect settings", "Native reconnect lighting",
                           "Native reconnect sensors", "Native reconnect intel"}
                ready = next((p["name"] for p in library["profiles"]
                              if p["content"]["kind"] == "screen"
                              and p["name"] in markers - restarted_pages), None)
                if ready:
                    stop()
                    # More than two GUI polling intervals; JS must observe offline.
                    time.sleep(4)
                    start()
                    restarted_pages.add(ready)
                time.sleep(.1)
            code = proc.returncode
        except BaseException:
            if proc.poll() is None:
                os.killpg(proc.pid, 15)
                try:
                    proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid, 9)
                    proc.wait()
            raise
    result = json.loads((gui_runtime / "result.json").read_text())
    if not result.get("ok"):
        result["profile_fault_counts"] = {"rejected": proxy.profile_rejected, "requests": proxy.profile_extra_requests, "dropped": proxy.profile_extra_dropped}
        (root / "failure.json").write_text(json.dumps(result, ensure_ascii=False, indent=2))
    assert code == 0 and result["ok"], result
    assert len(restarted_pages) == 6 and result["native_display_reconnect"] and result["native_controls_reconnect"], "native reconnect not exercised"
    assert result["native_saved_dependency_recovery"] and len(dependency_faults) == 8, "saved dependency recovery missing"
    result["dependency_faults"] = sorted(dependency_faults)
    assert result["native_restart_dependency_recovery"] and len(restart_dependency_faults) == 4, "restart dependency acceptance missing"
    result["restart_dependency_faults"] = sorted(restart_dependency_faults)
    assert result["native_combined_consumers"] and len(consumer_faults) == 11
    result["consumer_faults"] = sorted(consumer_faults)
    assert result["native_rgb_transport_recovery"] and not (root / "fail-pump-rgb").exists()
    result["rgb_transport_observations"] = transport_observations
    assert result["native_lcd_pwm_transport_recovery"]
    assert not (root / "fail-lcd").exists() and not (root / "fail-fan-pwm").exists()
    assert result["native_aged_sensor_recovery"]
    assert not (root / "freeze-sensors").exists() and not (root / "cpu-hot").exists()
    assert result["native_active_sensor_recovery"] and len(sensor_faults) == 2, "active sensor recovery not exercised"
    result["sensor_faults"] = sorted(sensor_faults)
    assert result["native_cpu_fan_mapping"], "explicit CPU fan mapping not exercised"
    assert result["native_display_fonts"], "new bundled fonts not exercised"
    assert result["native_noto_coverage"], "Noto coverage layer not exercised"
    assert result["native_gradient_background"], "native gradient workflow missing"
    assert result["native_intel_sources"], "Intel source selection/restart missing"
    assert result["native_sensor_defaults"], "native source workflow missing"
    assert result["native_recommended_defaults"], "native recommended CPU/GPU defaults workflow missing"
    assert result["native_lost_reply"], "lost reply scenario missing"
    assert proxy.requests == 1 and proxy.dropped == 1, "ambiguous mutation was retried"
    assert result["native_write_timeout"] and proxy.timeout_requests == 1 and proxy.withheld == 1, "timeout mutation was retried or not exercised"
    assert result["native_text_fallback"], "native typography scenario missing"
    assert result["native_theme_lost_reply"] and result["native_clear_lost_reply"], "theme/clear lost reply scenarios missing"
    assert proxy.theme_requests == 1 and proxy.theme_dropped == 1, "theme replacement retried or fault missing"
    assert proxy.clear_requests == 1 and proxy.clear_dropped == 1, "clear retried or fault missing"
    result["theme_reply_proxy"] = {"requests": proxy.theme_requests, "dropped_responses": proxy.theme_dropped}
    result["clear_reply_proxy"] = {"requests": proxy.clear_requests, "dropped_responses": proxy.clear_dropped}
    assert result["native_profile_lost_reply"], "profile lost reply scenario missing"
    assert proxy.profile_requests == 1 and proxy.profile_dropped == 1, "profile save retried or fault not exercised"
    assert result["native_profile_extra_faults"], "profile mutation faults missing"
    assert proxy.profile_extra_requests == {"rename": 1, "reset": 1, "delete": 1}, "profile mutations retried"
    assert proxy.profile_extra_dropped == proxy.profile_extra_requests, "profile fault not exercised"
    assert proxy.profile_rejected == 1, "pre-forward rejection retried or missing"
    result["profile_extra_proxy"] = {"requests": proxy.profile_extra_requests, "dropped": proxy.profile_extra_dropped, "rejected_before_forward": proxy.profile_rejected}
    result["profile_reply_proxy"] = {"requests": proxy.profile_requests, "dropped_responses": proxy.profile_dropped}
    assert result["native_source_lost_reply"], "source lost reply scenario missing"
    assert proxy.source_requests == 1 and proxy.source_dropped == 1 and proxy.source_total == 5, "source writes were retried or recovery missing"
    result["source_reply_proxy"] = {"target_requests": proxy.source_requests, "dropped_responses": proxy.source_dropped, "total_writes": proxy.source_total}
    assert proxy.font_begins == 3 and proxy.font_finishes == 2, "font upload restarted after ambiguous Finish"
    assert proxy.font_dropped == 1 and proxy.font_status_reads == 1, "font Finish recovery not exercised"
    assert result["theme"]["layers"][3]["style"]["font_asset"] == proxy.font_asset, "recovered font differs from committed asset"
    assert len(list((root / "settings/theme-fonts").glob("*.font"))) == 3, "duplicate font asset persisted"
    assert result["native_font_partial_upload"] and proxy.partial_chunks == 1 and proxy.partial_cancelled, "partial upload continued or leaked session"
    result["partial_font_proxy"] = {"chunks": proxy.partial_chunks, "cancelled": proxy.partial_cancelled}
    result["native_font_lost_finish"] = True
    result["font_reply_proxy"] = {"begins": proxy.font_begins, "finishes": proxy.font_finishes,
                                  "dropped_responses": proxy.font_dropped, "status_reads": proxy.font_status_reads,
                                  "asset": proxy.font_asset}
    assert result['native_cleanup_partial_retry'] and len(partial_cleanup_faults) == 4
    result['partial_cleanup_faults'] = sorted(partial_cleanup_faults)
    assert not (root / 'fail-unlink').exists()
    for folder, suffix in ((image_root, '.png'), (font_orphan.parent, '.font')):
        assert all(not (folder / (key * 32 + suffix)).exists() for key in ('f', '8')), 'retry left orphan files'
    assert result["native_cleanup_corrupt_profiles"]
    assert proxy.cleanup_profile_checks == ["cleanup_theme_assets", "cleanup_theme_fonts"]
    result["cleanup_profile_checks"] = proxy.cleanup_profile_checks
    assert not proxy.errors, proxy.errors
    result["lost_reply_proxy"] = {"requests": proxy.requests, "dropped_responses": proxy.dropped}
    result["timeout_proxy"] = {"requests": proxy.timeout_requests, "withheld_responses": proxy.withheld}
    first = json.loads((root / "stats.json").read_text())["jpeg_starts"]
    time.sleep(2.5)
    second = json.loads((root / "stats.json").read_text())["jpeg_starts"]
    assert second > first, "LCD JPEG stream stopped when GUI closed"
    stop()
    start()
    devices = []
    for _ in range(100):
        devices = rpc("list_devices", {})
        if devices:
            break
        time.sleep(.1)
    dev = devices[0]["id"]
    assert rpc("get_lcd_state", {"dev": dev})["source"] == "Theme"
    assert rpc("get_lcd_theme", {"dev": dev}) == result["theme"]
    assert rpc("get_sensor_defaults", {})["config"] == result["sensor_defaults"], "source choices changed after restart"
    assert rpc("list_profiles", {}) == result["profiles"], "profile library changed after restart"
    assert result["native_cleanup_corrupt_root"] and len(cleanup_faults) == 2, "cleanup corruption recovery missing"
    result["cleanup_faults"] = sorted(cleanup_faults)
    assert offline_settings.read_bytes() == offline_saved, "offline settings not restored"
    assert result["native_image_cleanup"], "native image cleanup not exercised"
    assert not (image_root / (image_keys["orphan"] + ".png")).exists(), "orphan image survived delete"
    assert result["native_draft_session_retention"], "draft/session retention not exercised"
    assert result["native_font_retention"], "font retained-reference acceptance missing"
    for kind in ("offline", "profile"):
        assert (font_orphan.parent / (font_keys[kind] + ".font")).read_bytes() == font_bytes, kind + " font changed or deleted"
        assert (image_root / (image_keys[kind] + ".png")).is_file(), kind + " image deleted"
        assert rpc("preview_lcd_theme", {"theme": retained_image_theme(image_keys[kind])}), kind + " image unreadable after restart"
    assert result["native_font_cleanup"] and not font_orphan.exists(), "native font cleanup failed"
    assert result["native_font_import"] and result["native_font_retry"] and result["native_font_parts"], "native font import/retry/parts workflow not exercised"
    controls = result["controls"]
    cooling = rpc("get_cooling_state", {"dev": dev})
    assert cooling["pump"]["requested"] == controls["pump"]
    assert cooling["fan"]["requested"] == controls["fan"]
    assert rpc("get_lcd_state", {"dev": dev})["requested"] == controls["lcd"]
    assert rpc("get_thermal_state", {"dev": dev})["requested"] == controls["thermal"]
    dynamic = rpc("get_dynamic_rgb", {"dev": dev})
    assert dynamic["requested"] == controls["dynamic"]
    assert dynamic["base_mode"] == controls["base_mode"]
    for _ in range(100):
        resumed_cooling = rpc("get_cooling_state", {"dev": dev})
        resumed_dynamic = rpc("get_dynamic_rgb", {"dev": dev})
        if (resumed_cooling["fan"]["applied_percent"] == 55
                and resumed_cooling["pump"]["applied_percent"] is not None
                and not resumed_cooling["pump"]["fallback_active"]
                and resumed_dynamic["applied"] is not None
                and not resumed_dynamic["degraded"]):
            break
        time.sleep(.1)
    else:
        raise AssertionError("controls did not resume applying after daemon restart")
    assert not resumed_cooling["fan"]["last_error"] and not resumed_cooling["pump"]["last_error"]
    assert not resumed_dynamic["last_error"]
    result.update(gui_closed_jpeg_frames=second - first, restart_replay=True, controls_restart_replay=True, controls_restart_applied=True, profiles_restart_replay=True)
    (root / "acceptance.json").write_text(json.dumps(result, indent=2))
    print(f"PASS native WebKitGTK → Unix IPC → MockBackend; evidence: {root}")
finally:
    if proxy is not None:
        proxy.close()
    stop()
    for log in logs:
        log.close()
    print(f"Logs: {root}")
