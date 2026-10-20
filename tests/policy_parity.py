#!/usr/bin/env python3
"""Compare executable Rust policy with functions extracted from the original C."""
import os, pathlib, re, shutil, subprocess, tempfile
root=pathlib.Path(__file__).resolve().parents[1]
c=(root/'tests/fixtures/fan_policy.c').read_text()
rust=f'#[path="{root}/kernel/policy.rs"] mod policy;\nfn main() {{\n'+'''
 for (index,profile) in [policy::CurveProfile::Conservative,policy::CurveProfile::Aggressive,policy::CurveProfile::Custom].into_iter().enumerate(){
  for temperature in -20..=140 {for cpu in [false,true] {println!("{index},{temperature},{},{}",cpu as u8,policy::fan_speed(temperature,cpu,profile,&policy::DEFAULT_POINTS[..5]));}}
 }
 let mut state=policy::Hysteresis::new();
 for temperature in [90,89,88,87,86,83,80,70,69,65,60,55,54,65,75,90]{let target=policy::fan_speed(temperature,true,policy::CurveProfile::Conservative,&policy::DEFAULT_POINTS[..5]);println!("h,{}",state.apply(temperature,target));}
}
'''
cc = os.environ.get('CC') or shutil.which('cc') or 'cc'
rustc = os.environ.get('RUSTC') or shutil.which('rustc') or 'rustc'
with tempfile.TemporaryDirectory(prefix='predatrix-policy-') as directory:
 directory=pathlib.Path(directory);(directory/'oracle.c').write_text(c);(directory/'probe.rs').write_text(rust)
 subprocess.run([cc,'-std=c11','-O2',str(directory/'oracle.c'),'-o',str(directory/'c')],check=True)
 subprocess.run([rustc,'--edition=2021','-Awarnings',str(directory/'probe.rs'),'-o',str(directory/'rust')],check=True)
 expected=subprocess.check_output([directory/'c'],text=True);actual=subprocess.check_output([directory/'rust'],text=True)
 if expected!=actual:
  import difflib
  print(''.join(difflib.unified_diff(expected.splitlines(True),actual.splitlines(True))));raise SystemExit(1)
 print(f'Original C and Rust fan policy match on {len(actual.splitlines())} cases, including cooling hysteresis.')
