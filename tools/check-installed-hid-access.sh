#!/usr/bin/env bash
# Run as the active desktop user after rebooting the clean Arch installation.
# Opens and closes the node; no report is read or sent.
set -euo pipefail
(( EUID != 0 )) || { echo 'Run as the desktop user, not root.' >&2; exit 1; }
matched=0
for node in /dev/hidraw*; do
  [[ -c "$node" ]] || continue
  props=$(udevadm info --query=property --name="$node")
  parent=$(readlink -f "/sys/class/hidraw/${node##*/}/device")
  while [[ "$parent" != / && ! -f "$parent/idVendor" ]]; do parent=$(dirname "$parent"); done
  [[ -f "$parent/idVendor" && $(<"$parent/idVendor") == 0416 && $(<"$parent/idProduct") == 7395 ]] || continue
  grep -Eq '^CURRENT_TAGS=.*:uaccess:' <<<"$props" || { echo "$node lacks active uaccess tag" >&2; exit 1; }
  python3 -c 'import os,sys; os.close(os.open(sys.argv[1],os.O_RDWR|os.O_NONBLOCK))' "$node"
  echo "$node: active uaccess tag; read/write open succeeded"
  matched=1
done
(( matched )) || { echo 'LCDV2 device not found' >&2; exit 1; }
