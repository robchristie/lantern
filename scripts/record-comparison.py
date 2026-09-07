#!/usr/bin/env python3
"""Record one real CLI invocation; never interpret or automate browser actions."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--run-dir', type=Path, required=True)
parser.add_argument('command', nargs=argparse.REMAINDER)
args = parser.parse_args()
command = args.command[1:] if args.command[:1] == ['--'] else args.command
if not command:
    parser.error('a real CLI command is required after --')
args.run_dir.mkdir(parents=True, exist_ok=True)
identity = str(uuid.uuid4())
started = datetime.now(timezone.utc).isoformat()
start = time.monotonic()
try:
    result = subprocess.run(command, capture_output=True, timeout=90)
    code, stdout, stderr = result.returncode, result.stdout, result.stderr
except subprocess.TimeoutExpired as exc:
    code, stdout, stderr = 124, exc.stdout or b'', (exc.stderr or b'') + b'\nRecorder timeout after 90 seconds\n'
except OSError as exc:
    code, stdout, stderr = 127, b'', str(exc).encode()
record = {'id': identity, 'argv': command, 'started_utc': started,
          'finished_utc': datetime.now(timezone.utc).isoformat(),
          'elapsed_seconds': time.monotonic()-start, 'exit_code': code}
for name, data in [('stdout', stdout), ('stderr', stderr)]:
    path = args.run_dir / f'{identity}.{name}'
    path.write_bytes(data)
    record[name] = {'path': str(path.resolve()), 'sha256': hashlib.sha256(data).hexdigest()}
with (args.run_dir / 'commands.jsonl').open('a') as log:
    log.write(json.dumps(record) + '\n')
sys.stdout.buffer.write(stdout)
sys.stderr.buffer.write(stderr)
sys.exit(code)
