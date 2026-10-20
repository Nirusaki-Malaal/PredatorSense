#!/usr/bin/env python3
"""Render GTK pages and exercise a draft edit against isolated mock IPC in Xvfb."""
import json
import os
import pathlib
import socket
import struct
import subprocess
import tempfile
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
ARTIFACTS = ROOT / 'artifacts'
ARTIFACTS.mkdir(exist_ok=True)


def run(*arguments):
    subprocess.run(arguments, check=True, timeout=8,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def exact(connection, length):
    data = b''
    while len(data) < length:
        block = connection.recv(length - len(data))
        if not block:
            raise EOFError('mock client disconnected')
        data += block
    return data


with tempfile.TemporaryDirectory(prefix='predator-ui-') as directory:
    path = pathlib.Path(directory) / 'control.sock'
    server = socket.socket(socket.AF_UNIX)
    server.bind(str(path))
    server.listen()
    status = json.loads((ROOT / 'tests/fixtures/ui_status.json').read_text())
    status['supported'] = list(status['values'])
    requests = []
    stop = threading.Event()

    def serve():
        while not stop.is_set():
            try:
                connection, _ = server.accept()
            except OSError:
                return
            try:
                with connection:
                    connection.settimeout(3)
                    length = struct.unpack('!I', exact(connection, 4))[0]
                    assert length <= 8192
                    request = json.loads(exact(connection, length))
                    requests.append(request)
                    if request.get('command') == 'set':
                        status['values'][request['feature']] = request['value']
                        if request['feature'] in ('four_zone_mode', 'per_zone_mode'):
                            status['saved'].pop('four_zone_mode', None)
                            status['saved'].pop('per_zone_mode', None)
                            status['saved'][request['feature']] = request['value']
                    data = json.dumps({'ok': True, 'error': None, 'status': status}).encode()
                    connection.sendall(struct.pack('!I', len(data)) + data)
            except (OSError, ValueError, EOFError):
                pass

    threading.Thread(target=serve, daemon=True).start()
    environment = os.environ.copy()
    for name in ['XDG_CURRENT_DESKTOP', 'XDG_SESSION_DESKTOP', 'WAYLAND_DISPLAY']:
        environment.pop(name, None)
    environment.update(
        PREDATOR_SENSE_SOCKET=str(path), GTK_A11Y='none', NO_AT_BRIDGE='1',
        GDK_BACKEND='x11', GTK_USE_PORTAL='0',
    )
    output = (ARTIFACTS / 'ui-test.log').open('w')
    binary = ROOT / os.environ.get('PREDATOR_UI_BINARY', 'target/debug/predator-sense')
    process = subprocess.Popen([binary], env=environment, stdout=output, stderr=output)
    try:
        window = None
        for _ in range(100):
            result = subprocess.run(['xdotool', 'search', '--name', '^Predator Sense$'],
                                    capture_output=True, text=True, timeout=3)
            if result.returncode == 0:
                window = result.stdout.splitlines()[0]
                break
            if process.poll() is not None:
                raise RuntimeError((ARTIFACTS / 'ui-test.log').read_text())
            time.sleep(.1)
        assert window, 'GTK window did not appear'
        time.sleep(3)
        run('import', '-window', window, str(ARTIFACTS / 'ui-dashboard.png'))
        # Coordinates are relative to this isolated 1120 × 800 window.
        for name, y in [('performance', 228), ('keyboard', 276), ('device', 324), ('diagnostics', 372)]:
            run('xdotool', 'mousemove', '--window', window, '110', str(y), 'click', '1')
            time.sleep(.3)
            run('import', '-window', window, str(ARTIFACTS / f'ui-{name}.png'))
            if name == 'keyboard':
                before = len([request for request in requests if request.get('feature') in ('four_zone_mode', 'per_zone_mode')])
                first = ARTIFACTS / 'ui-keyboard.png'
                first_frame = first.read_bytes()
                time.sleep(.35)
                run('import', '-window', window, str(ARTIFACTS / 'ui-keyboard-animation.png'))
                assert first_frame != (ARTIFACTS / 'ui-keyboard-animation.png').read_bytes(), 'animation did not advance'
                assert len([request for request in requests if request.get('feature') in ('four_zone_mode', 'per_zone_mode')]) == before, 'preview wrote hardware controls'
                # The top Apply action sends the chosen effect and a valid direction.
                run('xdotool', 'mousemove', '--window', window, '1020', '210', 'click', '1')
                for _ in range(30):
                    if any(request.get('feature') == 'four_zone_mode' for request in requests): break
                    time.sleep(.1)
                assert any(request.get('feature') == 'four_zone_mode' for request in requests), requests
                run('xdotool', 'mousemove', '--window', window, '1100', '650', 'click', '--repeat', '12', '--delay', '30', '5')
                time.sleep(.4)
                run('import', '-window', window, str(ARTIFACTS / 'ui-keyboard-palette.png'))
            if name == 'performance':
                # Keep an unsent edit across a status poll, then apply it through GTK.
                run('xdotool', 'mousemove', '--window', window, '942', '536', 'click', '1',
                    'key', 'ctrl+a', 'type', '37')
                time.sleep(1.4)
                # Panel construction is lazy; revisiting must preserve the draft.
                run('xdotool', 'mousemove', '--window', window, '110', '276', 'click', '1')
                time.sleep(.3)
                run('xdotool', 'mousemove', '--window', window, '110', '228', 'click', '1')
                time.sleep(.3)
                run('xdotool', 'mousemove', '--window', window, '600', '636', 'click', '1')
                expected = {'command': 'set', 'feature': 'fan_speed', 'value': '37,0'}
                for _ in range(30):
                    if expected in requests:
                        break
                    time.sleep(.1)
                assert expected in requests, requests
        status_text = pathlib.Path(f'/proc/{process.pid}/status').read_text()
        rss = next(line for line in status_text.splitlines() if line.startswith('VmRSS:'))
        memory = pathlib.Path(f'/proc/{process.pid}/smaps_rollup').read_text()
        pss = next(line for line in memory.splitlines() if line.startswith('Pss:'))
        private = sum(int(line.split()[1]) for line in memory.splitlines()
                      if line.startswith(('Private_Clean:', 'Private_Dirty:')))
        print(f'GTK rendered five pages; fan draft survived polling and applied through IPC; '
              f'{rss}; {pss}; private={private} KiB')
    finally:
        process.terminate()
        process.wait(timeout=5)
        stop.set()
        server.close()
        output.close()
