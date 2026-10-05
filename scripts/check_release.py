"""Bounded local release audit: PE hardening/imports and accidental private paths.

This is a targeted check, not a guarantee that every conceivable secret is absent.
"""
import json
import pathlib
import re
import struct
import sys

root = pathlib.Path(__file__).resolve().parent.parent
exe = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else root / 'target/release/voicestage.exe'
data = exe.read_bytes()
pe = struct.unpack_from('<I', data, 0x3c)[0]
assert data[pe:pe+4] == b'PE\0\0'
machine, count = struct.unpack_from('<HH', data, pe+4)
assert machine == 0x8664, 'Expected x64 PE'
optional_size = struct.unpack_from('<H', data, pe+20)[0]
optional = pe+24
assert struct.unpack_from('<H', data, optional)[0] == 0x20b
flags = struct.unpack_from('<H', data, optional+70)[0]
assert flags & 0x100, 'DEP/NX missing'
assert flags & 0x40, 'ASLR missing'
sections = []
for i in range(count):
    off = optional+optional_size+i*40
    virtual_size, virtual_address, raw_size, raw_pointer = struct.unpack_from('<IIII', data, off+8)
    sections.append((virtual_address, max(virtual_size, raw_size), raw_pointer))
def offset(rva):
    return next(raw+rva-va for va, size, raw in sections if va <= rva < va+size)
def string(rva):
    start=offset(rva)
    return data[start:data.index(b'\0', start)].decode('ascii')
import_rva = struct.unpack_from('<I', data, optional+112+8)[0]
pos=offset(import_rva)
imports=[]
while any(data[pos:pos+20]):
    imports.append(string(struct.unpack_from('<I',data,pos+12)[0]))
    pos+=20
assert b'UPX!' not in data, 'Unexpected executable packer'
home=str(pathlib.Path.home())
for prefix in [home, home.replace('\\','/')]:
    for encoding in ['utf-8','utf-16-le']:
        assert prefix.encode(encoding).lower() not in data.lower(), 'Private build path in executable'
issues=[]
for p in root.rglob('*'):
    if not p.is_file() or any(part in {'.git','target','dist','test-output'} for part in p.relative_to(root).parts):
        continue
    b=p.read_bytes()
    if b'\xef\xbf\xbd' in b:
        issues.append(f'{p.relative_to(root)}: Unicode replacement character')
    if home.encode().lower() in b.lower() or home.replace('\\','/').encode().lower() in b.lower():
        issues.append(f'{p.relative_to(root)}: private path')
    if re.search(rb'(?:gh[pousr]_[A-Za-z0-9]{30,}|sk-[A-Za-z0-9_-]{30,}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----)',b):
        issues.append(f'{p.relative_to(root)}: credential-like string')
assert not issues, '\n'.join(issues)
print(json.dumps({'x64':True,'DEP':True,'ASLR':True,'high_entropy_ASLR':bool(flags&0x20),'imports':imports,'exe_bytes':len(data),'private_path_scan':'pass','targeted_secret_scan':'pass'},indent=2))
