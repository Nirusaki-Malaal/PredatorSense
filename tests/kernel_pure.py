#!/usr/bin/env python3
"""Run queue and binary-format tests on the host without loading a driver."""
import pathlib
import shutil
import subprocess
import tempfile

root = pathlib.Path(__file__).resolve().parents[1]
rustc = shutil.which('rustc') or 'rustc'
with tempfile.TemporaryDirectory(prefix='predator-kernel-tests-') as directory:
    directory = pathlib.Path(directory)
    source = '#![allow(dead_code)]\n'
    for module in ['policy', 'thermal_state', 'event_queue', 'model']:
        source += f'#[path = "{root}/kernel/{module}.rs"] mod {module};\n'
    (directory / 'test.rs').write_text(source)
    subprocess.run([rustc, '--edition=2021', '--test', directory / 'test.rs', '-o', directory / 'test'], check=True)
    subprocess.run([directory / 'test'], check=True)
