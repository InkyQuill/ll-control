#!/usr/bin/env python3
"""Run GJS transport fixtures without installing an extension or touching a session."""
import argparse,os,signal,subprocess,tempfile
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('package',type=Path);parser.add_argument('--gjs',required=True);args=parser.parse_args()
with tempfile.TemporaryDirectory(prefix='lconnect-mini-gjs-feed-') as tmp:
 root=Path(tmp);(root/'bin').mkdir()
 env=dict(os.environ,LCONNECT_MINI_TEST_ROOT=str(root),PATH=str(root/'bin')+':'+os.environ.get('PATH',''),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),XDG_RUNTIME_DIR=str(root))
 process=subprocess.Popen([args.gjs,'-m',str(Path(__file__).with_name('feed.test.js')),str(args.package.resolve())],env=env,start_new_session=True)
 try:code=process.wait(timeout=35)
 finally:
  try:os.killpg(process.pid,signal.SIGTERM)
  except ProcessLookupError:pass
  if process.poll() is None:process.wait(timeout=5)
 raise SystemExit(code)
