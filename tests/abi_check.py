#!/usr/bin/env python3
"""Compare Rust bindgen layouts with the target kernel's C compiler, never guess."""
import argparse, pathlib, subprocess, tempfile, os
parser=argparse.ArgumentParser();parser.add_argument('headers',type=pathlib.Path);parser.add_argument('bindings',type=pathlib.Path);args=parser.parse_args()
root=pathlib.Path(__file__).resolve().parents[1]
fields={
 'wmi_device':['dev','setable'], 'wmi_buffer':['length','data'],
 'wmi_driver':['driver','id_table','min_event_size','no_singleton','probe','remove','shutdown','notify','notify_new'],
 'device':['kobj','driver_data'], 'platform_device':['dev'],
 'platform_driver':['probe','remove','shutdown','driver'],
 'input_dev':['name','phys','id','open'],
 'hwmon_ops':['is_visible','read'],
 'hwmon_channel_info':['type','config'], 'hwmon_chip_info':['ops','info'],
 'platform_profile_ops':['probe','profile_get','profile_set'],
 'acpi_device':['handle','dev'], 'kernel_param':['name','mod','ops','perm','arg'],
 'kernel_param_ops':['flags','set','get','free'], 'attribute':['name','mode'],
 'device_attribute':['attr','show','store'], 'attribute_group':['name','attrs'],
}
rename={('device_attribute','show'):'__bindgen_anon_1',('device_attribute','store'):'__bindgen_anon_2',('attribute_group','attrs'):'__bindgen_anon_2',('kernel_param','arg'):'__bindgen_anon_1'}
with tempfile.TemporaryDirectory(prefix='predatrix-abi-') as directory:
 directory=pathlib.Path(directory)
 c=[f'#include "{args.bindings.resolve().parent / "ffi.h"}"','extern int printf(const char *, ...);','int main(void) {']
 rust=[f'#[allow(warnings)] mod ffi {{ include!("{args.bindings.resolve()}"); }}','fn main() {']
 for name,members in fields.items():
  c.append(f'printf("{name} %lu %lu\\n", (unsigned long)sizeof(struct {name}), (unsigned long)_Alignof(struct {name}));')
  rust.append(f'println!("{name} {{}} {{}}",core::mem::size_of::<ffi::{name}>(),core::mem::align_of::<ffi::{name}>());')
  for member in members:
   rust_member=rename.get((name,member),member+'_' if member in ['mod','type'] else member)
   c.append(f'printf("{name}.{member} %lu\\n", (unsigned long)__builtin_offsetof(struct {name}, {member}));')
   rust.append(f'println!("{name}.{member} {{}}",core::mem::offset_of!(ffi::{name},{rust_member}));')
 c.append('return 0; }');rust.append('}')
 (directory/'probe.c').write_text('\n'.join(c));(directory/'probe.rs').write_text('\n'.join(rust))
 headers=args.headers.resolve();flags=['-std=gnu11','-fcf-protection=branch','-D__KERNEL__','-DMODULE','-funsigned-char','-fshort-wchar','-fms-extensions','-O2']
 for path in ['arch/x86/include','arch/x86/include/generated','include','arch/x86/include/uapi','arch/x86/include/generated/uapi','include/uapi','include/generated/uapi']:flags+=['-I'+str(headers/path)]
 for path in ['include/linux/compiler-version.h','include/linux/kconfig.h','include/linux/compiler_types.h']:flags+=['-include',str(headers/path)]
 subprocess.run([os.environ.get('CC','/usr/bin/cc'),*flags,str(directory/'probe.c'),'-o',str(directory/'c')],check=True)
 subprocess.run(['/usr/bin/rustc','--edition=2021','-Awarnings',str(directory/'probe.rs'),'-o',str(directory/'rust')],check=True)
 expected=subprocess.check_output([directory/'c'],text=True);actual=subprocess.check_output([directory/'rust'],text=True)
 if expected!=actual:
  import difflib
  print(''.join(difflib.unified_diff(expected.splitlines(True),actual.splitlines(True),fromfile='C ABI',tofile='Rust ABI')));raise SystemExit(1)
 print(f'ABI layouts match for {len(fields)} kernel structures and {sum(map(len,fields.values()))} accessed fields.')
