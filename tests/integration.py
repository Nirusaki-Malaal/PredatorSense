#!/usr/bin/env python3
"""Native IPC, bounded clients and persistence against isolated fake sysfs."""
import json
import os
import pathlib
import socket
import statistics
import struct
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = ROOT / os.environ.get('PREDATOR_DAEMON_BINARY', 'target/debug/predator-daemon')


def receive(connection):
    def exact(length):
        data = b''
        while len(data) < length:
            block = connection.recv(length - len(data))
            assert block, 'premature EOF'
            data += block
        return data
    length = struct.unpack('!I', exact(4))[0]
    assert length <= 8192
    return json.loads(exact(length))


def request(path, command):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(3)
        connection.connect(str(path))
        data = json.dumps(command).encode()
        connection.sendall(struct.pack('!I', len(data)) + data)
        return receive(connection)


def wait_for(predicate):
    for _ in range(100):
        if predicate():
            return
        time.sleep(.02)
    raise AssertionError('condition timed out')


with tempfile.TemporaryDirectory(prefix='predator-native-test-') as directory:
    directory = pathlib.Path(directory)
    sysfs = directory / 'sys'
    device = sysfs / 'devices/platform/acer-wmi/predator_sense'
    keyboard = device.parent / 'four_zoned_kb'
    device.mkdir(parents=True)
    keyboard.mkdir()
    features = {
        'backlight_timeout': '1', 'battery_calibration': '0',
        'battery_limiter': '0', 'boot_animation_sound': '0', 'fan_speed': '0,0',
        'thermal_fan_mode': 'auto', 'fan_curve_profile': 'conservative',
        'fan_curve_points': '55:0:0,65:35:35,75:55:60,85:80:85,90:100:100',
        'lcd_override': '0', 'usb_charging': '0',
    }
    for name, value in features.items():
        (device / name).write_text(value)
    (keyboard / 'four_zone_mode').write_text('0,0,100,0,255,0,0')
    (keyboard / 'per_zone_mode').write_text('ff0000,00ff00,0000ff,ffffff,100')
    acpi = sysfs / 'firmware/acpi'
    acpi.mkdir(parents=True)
    (acpi / 'platform_profile').write_text('balanced')
    (acpi / 'platform_profile_choices').write_text('low-power quiet balanced balanced-performance performance')
    power = sysfs / 'class/power_supply/AC'
    power.mkdir(parents=True)
    (power / 'type').write_text('Mains')
    (power / 'online').write_text('1')
    model = sysfs / 'class/dmi/id'
    model.mkdir(parents=True)
    (model / 'product_name').write_text('Predator PHN16-71')
    (model / 'sys_vendor').write_text('Acer')
    native = directory / 'control.sock'
    state = directory / 'state/preferences.json'
    log = directory / 'daemon.log'

    def start(state_path=state):
        with log.open('ab') as output:
            process = subprocess.Popen([
                BINARY, '--sysfs', sysfs,
                '--socket', native, '--state', state_path,
            ], stdout=output, stderr=output)
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError(log.read_text())
            try:
                if request(native, {'command': 'status'})['ok']:
                    return process
            except OSError:
                pass
            time.sleep(.02)
        raise RuntimeError('startup timed out')

    def set_value(feature, value):
        result = request(native, {'command': 'set', 'feature': feature, 'value': value})
        assert result['ok'], result
        return result

    process = start()
    try:
        for feature in ['backlight_timeout', 'battery_limiter', 'battery_calibration', 'boot_animation_sound', 'lcd_override']:
            set_value(feature, '1')
            assert (device / feature).read_text().strip() == '1'
        set_value('usb_charging', '20')
        set_value('profile', 'balanced-performance')
        set_value('fan_speed', '0,65')
        points = '40:20:30,80:90:100'
        assert request(native, {'command': 'configure_curve', 'profile': 'custom', 'points': points})['ok']
        assert (device / 'fan_curve_points').read_text().strip() == points
        assert (device / 'thermal_fan_mode').read_text().strip() == 'curve'
        assert not request(native, {'command': 'configure_curve', 'profile': 'custom', 'points': 'bad'})['ok']
        assert (device / 'fan_curve_points').read_text().strip() == points
        assert not request(native, {'command': 'set', 'feature': '../escape', 'value': '1'})['ok']
        # RGB status reads use the cache. An explicit refresh updates it.
        effect = keyboard / 'four_zone_mode'
        old = request(native, {'command': 'status'})['status']['values']['four_zone_mode']
        effect.write_text('3,5,42,1,0,0,0')
        assert request(native, {'command': 'status'})['status']['values']['four_zone_mode'] == old
        assert request(native, {'command': 'refresh'})['status']['values']['four_zone_mode'] == '3,5,42,1,0,0,0'
        # A fragmented client must not delay a healthy request until its timeout.
        with socket.socket(socket.AF_UNIX) as stalled:
            stalled.connect(str(native))
            stalled.sendall(b'\x00')
            begin = time.monotonic()
            assert request(native, {'command': 'status'})['ok']
            assert time.monotonic() - begin < .20
        with socket.socket(socket.AF_UNIX) as invalid:
            invalid.connect(str(native))
            invalid.sendall(struct.pack('!I', 0xffffffff))
            assert not receive(invalid)['ok']
        lighting = 'ff0000,00ff00,0000ff,ffffff,73'
        set_value('per_zone_mode', lighting)
        wait_for(lambda: state.exists() and json.loads(state.read_text()).get('per_zone_mode') == lighting)
        saved = json.loads(state.read_text())
        assert not any('calibration' in key for key in saved)
        assert state.stat().st_mode & 0o777 == 0o600
        assert saved['profile_ac'] == 'balanced-performance'
        # Capture a physical button/profile change even without GUI commands.
        (acpi / 'platform_profile').write_text('quiet')
        wait_for(lambda: json.loads(state.read_text()).get('profile_ac') == 'quiet')
        # Profile restrictions and preferences follow the active supply.
        (power / 'online').write_text('0')
        time.sleep(.65)
        set_value('profile', 'low-power')
        (power / 'online').write_text('1')
        wait_for(lambda: (acpi / 'platform_profile').read_text().strip() == 'quiet')
        before_fds = len(list(pathlib.Path(f'/proc/{process.pid}/fd').iterdir()))
        def rss():
            return int(next(line.split()[1] for line in pathlib.Path(f'/proc/{process.pid}/status').read_text().splitlines() if line.startswith('VmRSS:')))
        before_rss = rss()
        timings = []
        for _ in range(1000):
            begin = time.perf_counter()
            assert request(native, {'command': 'status'})['ok']
            timings.append((time.perf_counter() - begin) * 1000)
        after_fds = len(list(pathlib.Path(f'/proc/{process.pid}/fd').iterdir()))
        after_rss = rss()
        assert after_fds <= before_fds + 1
        assert after_rss <= before_rss + 2048
        print(f'1000 cached requests: p50={statistics.median(timings):.3f}ms p95={sorted(timings)[949]:.3f}ms; RSS {before_rss}→{after_rss}KiB; FDs {before_fds}→{after_fds}')
        process.terminate()
        process.wait(timeout=5)
        (keyboard / 'per_zone_mode').write_text('000000,000000,000000,000000,0')
        (device / 'battery_calibration').write_text('0')
        process = start()
        assert (keyboard / 'per_zone_mode').read_text().strip() == lighting
        assert (device / 'battery_calibration').read_text() == '0'
        process.terminate()
        process.wait(timeout=5)
        # Corruption is backed up; it must not prevent hardware control.
        state.write_text('{broken')
        process = start()
        set_value('battery_limiter', '0')
        wait_for(lambda: bool(list(state.parent.glob('preferences.corrupt.*'))))
        assert next(state.parent.glob('preferences.corrupt.*')).read_text() == '{broken'
        process.terminate()
        process.wait(timeout=5)
        # A failed startup restore must not replace the desired profile.
        restore_state = directory / 'restore/preferences.json'
        restore_state.parent.mkdir()
        restore_state.write_text(json.dumps({'profile_ac': 'balanced-performance'}))
        profile = acpi / 'platform_profile'
        profile.unlink()
        process = start(restore_state)
        time.sleep(.65)
        result = request(native, {'command': 'status'})
        assert result['status']['errors'], result
        assert json.loads(restore_state.read_text())['profile_ac'] == 'balanced-performance'
        profile.write_text('balanced')
        assert request(native, {'command': 'restore'})['ok']
        assert profile.read_text().strip() == 'balanced-performance'
        process.terminate()
        process.wait(timeout=5)
        # Retry a failed disk flush without requiring a second user edit.
        blocked = directory / 'blocked'
        blocked.write_text('not a directory')
        retry_state = blocked / 'preferences.json'
        process = start(retry_state)
        set_value('per_zone_mode', lighting)
        time.sleep(.20)
        blocked.unlink()
        blocked.mkdir()
        wait_for(lambda: retry_state.exists() and json.loads(retry_state.read_text()).get('per_zone_mode') == lighting)
        print('Native controls, client isolation, per-power restoration, lighting restart, calibration exclusion, corrupt-file recovery, failed-restore retention and background-flush retry passed.')
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
    (model / 'product_name').write_text('Predator PHN16-72')
    result = subprocess.run([BINARY, '--check', '--sysfs', sysfs], capture_output=True, text=True)
    assert result.returncode != 0 and 'Unsupported hardware' in result.stderr
