#!/usr/bin/env python3
"""Measure only process startup/help; does NOT measure interactive TUI startup."""
import argparse, json, platform, statistics, subprocess, time
from pathlib import Path

p=argparse.ArgumentParser()
p.add_argument('--binary',default='target/release/aether')
p.add_argument('--samples',type=int,default=30)
p.add_argument('--output',default='docs/benchmark-results.json')
a=p.parse_args()
if a.samples<2:p.error('samples must be >= 2')
values=[]
for _ in range(a.samples):
    start=time.perf_counter_ns()
    subprocess.run([a.binary,'--help'],stdout=subprocess.DEVNULL,check=True)
    values.append((time.perf_counter_ns()-start)/1e6)
result={'benchmark':'warm process startup + CLI help + process exit; NOT TUI startup',
        'platform':platform.platform(),'machine':platform.machine(),
        'version':subprocess.check_output([a.binary,'--version'],text=True).strip(),
        'samples_ms':values,'median_ms':statistics.median(values),
        'p95_ms':sorted(values)[int((len(values)-1)*.95)],
        'conditions':'sequential subprocesses; OS cache warm; no network, database, TUI or model inference; no OpenCode comparison'}
Path(a.output).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='samples_ms'},indent=2))
