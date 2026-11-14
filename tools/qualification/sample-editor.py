"""Sample explicit GUI, WebKit and daemon PIDs during the production editor run."""
import argparse
import csv
import os
from pathlib import Path
import sys
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("pids", type=int, nargs="+")
parser.add_argument("--seconds", type=int, default=1800)
args = parser.parse_args()
if args.seconds < 1:
    parser.error("duration must be positive")
hz = os.sysconf("SC_CLK_TCK")
writer = csv.writer(sys.stdout)
writer.writerow(("elapsed_s", "pid", "cpu_percent_one_core", "rss_kib"))
start = time.monotonic()
previous = {}
while True:
    for pid in args.pids:
        stat = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        now = time.monotonic()
        ticks = int(stat[11]) + int(stat[12])
        if pid in previous:
            old_time, old_ticks, identity = previous[pid]
            if identity != stat[19]:
                sys.exit(f"PID {pid} was reused")
            writer.writerow((round(now-start, 3), pid, round(100*(ticks-old_ticks)/hz/(now-old_time), 2),
                             int(stat[21])*os.sysconf("SC_PAGE_SIZE")//1024))
        previous[pid] = now, ticks, stat[19]
    sys.stdout.flush()
    if now-start >= args.seconds:
        break
    time.sleep(1)
