#!/usr/bin/env python3
"""Build a local candidate extension package; never install or enable it."""
import argparse,json,re,shutil,subprocess
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output',type=Path,required=True)
parser.add_argument('--shell-version',action='append',required=True,help='Explicit candidate version to validate; does not declare a tested support matrix')
args=parser.parse_args()
assert all(re.fullmatch(r'[0-9]{2,3}',v) and int(v)>=45 for v in args.shell_version),'Use GNOME ES-module API candidate versions >=45'
output=args.output.resolve();package=output/'lconnect-mini@inkyquill.net'
if package.exists() and not (package/'.lconnect-mini-build').is_file():
 raise SystemExit('Output directory is not owned by this builder; preserved')
package.mkdir(parents=True,exist_ok=True);(package/'.lconnect-mini-build').touch()
for source in (repo/'integrations/gnome/src').glob('*.js'):shutil.copy2(source,package/source.name)
(package/'schemas').mkdir(exist_ok=True)
for source in (repo/'integrations/gnome/schemas').glob('*.xml'):shutil.copy2(source,package/'schemas'/source.name)
model=output/'common';model.mkdir(exist_ok=True)
subprocess.run(['node',str(repo/'desktop/node_modules/typescript/bin/tsc'),str(repo/'integrations/common/mini.ts'),'--strict','--target','ES2017','--module','ES2020','--declaration','--skipLibCheck','--outDir',str(model)],check=True)
shutil.copy2(model/'mini.js',package/'mini.mjs')
metadata=json.loads((repo/'integrations/gnome/metadata.in.json').read_text());metadata['shell-version']=list(dict.fromkeys(args.shell_version));(package/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
subprocess.run(['glib-compile-schemas','--strict',str(package/'schemas')],check=True)
for source in package.glob('*.js'):subprocess.run(['node','--check',str(source)],check=True)
print(package)
