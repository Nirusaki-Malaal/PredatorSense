#!/usr/bin/env python3
"""Measure the real desktop renderer against mock IPC, without touching firmware.

Unlike Xvfb, running this on the current desktop can exercise the GPU driver.
The temporary window is closed after sampling. RSS/PSS/private are all reported.
"""
import argparse
import json
import os
import pathlib
import re
import socket
import struct
import subprocess
import tempfile
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=pathlib.Path, default=ROOT / 'target/release/predator-sense')
parser.add_argument('--seconds', type=float, default=10)
parser.add_argument('--output', type=pathlib.Path, default=ROOT / 'artifacts/ui-memory.json')
args = parser.parse_args()
assert 3 <= args.seconds <= 120
args.output.parent.mkdir(parents=True, exist_ok=True)


def exact(connection, length):
    data = b''
    while len(data) < length:
        block = connection.recv(length - len(data))
        if not block:
            raise EOFError
        data += block
    return data


def memory(pid):
    lines = pathlib.Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines()
    fields = {line.split(':')[0]: int(line.split()[1]) for line in lines[1:]}
    return {
        'rss_kib': fields['Rss'],
        'pss_kib': fields['Pss'],
        'private_kib': fields['Private_Clean'] + fields['Private_Dirty'],
    }


with tempfile.TemporaryDirectory(prefix='predator-gpu-memory-') as directory:
    path = pathlib.Path(directory) / 'control.sock'
    server = socket.socket(socket.AF_UNIX)
    server.bind(str(path))
    server.listen()
    stop = threading.Event()
    status = json.loads((ROOT / 'tests/fixtures/ui_status.json').read_text())
    status['supported'] = list(status['values'])
    requests = []

    def serve():
        while not stop.is_set():
            try:
                connection, _ = server.accept()
            except OSError:
                return
            try:
                with connection:
                    connection.settimeout(2)
                    length = struct.unpack('!I', exact(connection, 4))[0]
                    assert length <= 8192
                    request = json.loads(exact(connection, length))
                    requests.append(request)
                    if request['command'] == 'status':
                        response = {'ok': True, 'error': None, 'status': status}
                    else:
                        response = {'ok': False, 'error': 'Memory test uses mock controls; hardware is unchanged.', 'status': None}
                    data = json.dumps(response).encode()
                    connection.sendall(struct.pack('!I', len(data)) + data)
            except (OSError, ValueError, EOFError):
                pass

    threading.Thread(target=serve, daemon=True).start()
    environment = dict(os.environ, PREDATOR_SENSE_SOCKET=str(path),
                       PREDATOR_SENSE_DIAGNOSTICS='1', GSK_DEBUG='renderer',
                       GTK_A11Y='none', NO_AT_BRIDGE='1')
    log_path = args.output.with_suffix('.log')
    with log_path.open('w') as log:
        process = subprocess.Popen([args.binary.resolve()], env=environment, stdout=log, stderr=log)
        try:
            samples = []
            started = time.monotonic()
            while time.monotonic() - started < args.seconds:
                if process.poll() is not None:
                    raise RuntimeError(log_path.read_text())
                if time.monotonic() - started >= 2:
                    samples.append(memory(process.pid))
                time.sleep(.25)
            mappings = pathlib.Path(f'/proc/{process.pid}/maps').read_text()
            libraries = sorted(set(re.findall(r'/\S*(?:nvidia|LLVM|gallium|vulkan)\S*', mappings)))
            assert requests, 'no status request was processed'
            renderer = re.search(r'Predator Sense renderer: (\w+)', log_path.read_text())
            report = {
                'binary': str(args.binary.resolve()),
                'renderer': renderer.group(1) if renderer else 'see log',
                'display': environment.get('WAYLAND_DISPLAY') or environment.get('DISPLAY'),
                'graphics_overrides': {key: environment[key] for key in
                                       ['GSK_RENDERER', 'GDK_DISABLE'] if key in environment},
                'last': samples[-1],
                'peak': {key: max(sample[key] for sample in samples) for key in samples[0]},
                'loaded_graphics_libraries': libraries,
                'samples': samples,
            }
            args.output.write_text(json.dumps(report, indent=2) + '\n')
            print(json.dumps({key: report[key] for key in ['renderer', 'graphics_overrides', 'last', 'peak']}, indent=2))
        finally:
            process.terminate()
            process.wait(timeout=5)
            stop.set()
            server.close()
