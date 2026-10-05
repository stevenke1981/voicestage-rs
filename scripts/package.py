"""Create a portable ZIP with explicit files, notices and checksums."""
import hashlib
import pathlib
import shutil
import json
import subprocess
import zipfile

root=pathlib.Path(__file__).resolve().parent.parent
out=root/'dist'
out.mkdir(exist_ok=True)
files={'voicestage.exe': root/'target/release/voicestage.exe'}
for name in ['README.md','LICENSE','THIRD_PARTY_NOTICES.md','THIRD_PARTY_LICENSES.txt','RUST-COPYRIGHT-library.html','example-project.json']:
    files[name]=root/name
for p in (root/'docs').rglob('*'):
    if p.is_file():
        files[p.relative_to(root).as_posix()]=p
payload={name:p.read_bytes() for name,p in files.items()}
commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
payload['BUILD_INFO.json']=json.dumps({'version':'0.1.0','source_commit':commit,'target':'x86_64-pc-windows-gnu','profile':'opt3/fat-LTO/codegen-units1/strip/panic-abort','real_api_tested':False,'independent_visual_QA':'pending; supported desktop tools unavailable'},indent=2).encode()
internal=''.join(f'{hashlib.sha256(data).hexdigest()}  {name}\n' for name,data in sorted(payload.items()))
archive=out/'VoiceStage-v0.1.0-windows-x64-portable.zip'
with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
    for name,data in sorted(payload.items()):
        info=zipfile.ZipInfo('VoiceStage/'+name,date_time=(2026,10,5,0,0,0))
        info.compress_type=zipfile.ZIP_DEFLATED
        z.writestr(info,data)
    z.writestr('VoiceStage/SHA256SUMS.txt',internal)
checksums=f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n{hashlib.sha256(files["voicestage.exe"].read_bytes()).hexdigest()}  voicestage.exe\n'
(out/'SHA256SUMS.txt').write_text(checksums,encoding='ascii')
shutil.copyfile(root/'docs/RELEASE_NOTES.md',out/'RELEASE_NOTES.md')
with zipfile.ZipFile(archive) as z:
    assert z.testzip() is None
    assert z.read('VoiceStage/voicestage.exe')==files['voicestage.exe'].read_bytes()
print(f'{archive.name}: {archive.stat().st_size} bytes')
print(checksums)
