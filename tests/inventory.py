#!/usr/bin/env python3
"""Check the intentional PHN16-71 surface and safe userspace boundary."""
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
sysfs = (ROOT / 'kernel/sysfs.rs').read_text()
attributes = set(re.findall(r'= &attribute\(c"([^"]+)"\)', sysfs))
assert len(attributes) == 12, attributes
assert 'group(c"predator_sense"' in sysfs
assert 'group(c"four_zoned_kb"' in sysfs
assert 'nitro_sense' not in sysfs
for name in ['legacy.rs', 'models.rs', 'persistence.rs']:
    assert not (ROOT / 'kernel' / name).exists()
for name in ['compat.rs', 'legacy_server.rs', 'logging.rs']:
    assert not (ROOT / 'crates/daemon/src' / name).exists()
for path in (ROOT / 'crates').rglob('*.rs'):
    assert not re.search(r'\bunsafe\s*(?:\{|impl|trait|fn)', path.read_text()), path
for path in (ROOT / 'kernel').glob('*.rs'):
    assert not re.search(r'unsafe\s+impl', path.read_text()), path
artifact = ROOT / 'artifacts/kernel' / subprocess.check_output(['uname', '-r'], text=True).strip() / 'predatrix_sense.ko'
def info(field):
    return subprocess.check_output(['modinfo', '-F', field, artifact], text=True).strip()
assert info('version') == '0.6.1'
assert {line.split(':', 1)[0] for line in info('parm').splitlines()} == {'cycle_gaming_thermal_profile'}
assert info('license') and info('vermagic')
sections = subprocess.check_output(['readelf', '-SW', artifact], text=True)
assert len(re.findall(r'\]\s+\.modinfo\s', sections)) == 1
print('PHN16-71 inventory: 12 attributes, two groups, one parameter; no authored userspace unsafe or kernel unsafe impl; module metadata valid.')
