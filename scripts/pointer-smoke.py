#!/usr/bin/env python3
"""Exercise pointer dispatch and crop pixels in an existing disposable browser.

Run from the repository root after cargo build --workspace. The browser must be
able to reach --fixture-host at --port; use --bind 0.0.0.0 and a configured host
gateway name for a container. The caller owns browser creation and cleanup.
"""
import argparse
import functools
import http.server
import json
from pathlib import Path
import struct
import subprocess
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lantern', default='target/debug/lantern')
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--target-id', required=True, help='Disposable page target to navigate')
    parser.add_argument('--bind', default='127.0.0.1')
    parser.add_argument('--fixture-host', default='127.0.0.1')
    parser.add_argument('--port', type=int, default=8765)
    parser.add_argument('--output-dir', default='.smoogle/artifacts/pointer-smoke')
    args = parser.parse_args()
    output = Path(args.output_dir)
    output.mkdir(parents=True, exist_ok=True)
    fixture = Path(__file__).resolve().parent / 'fixtures'
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(fixture))
    server = http.server.ThreadingHTTPServer((args.bind, args.port), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    results = []

    def run(*command):
        result = subprocess.run([args.lantern, *command, '--endpoint', args.endpoint,
                                 '--target-id', args.target_id, '--json'],
                                check=True, capture_output=True, text=True, timeout=40)
        value = json.loads(result.stdout)
        assert value.get('ok'), value
        results.append(value)
        return value

    try:
        run('open', f'http://{args.fixture_host}:{server.server_port}/pointer-smoke.html')
        run('wait', 'ready', '--state', 'complete', '--timeout-ms', '5000')
        run('hover', '--selector', '#surface', '--timeout-ms', '5000')
        run('wheel', '--selector', '#surface', '--dy', '80', '--timeout-ms', '5000')
        started = time.monotonic()
        run('pointer-drag', '--selector', '#surface', '--dx', '60', '--dy', '20',
            '--duration-ms', '1200', '--timeout-ms', '5000')
        elapsed = time.monotonic() - started
        assert elapsed >= 1.2, f'Drag finished early: {elapsed}s'
        expected = {'hover-observed', 'wheel-observed', 'drag-start-observed',
                    'drag-move-observed', 'drag-end-observed'}
        deadline = time.monotonic() + 5
        while True:
            dom = run('dom', '--depth', '6', '--max-nodes', '80')
            observed = json.dumps(dom)
            if all(marker in observed for marker in expected):
                break
            assert time.monotonic() < deadline, dom
            time.sleep(0.1)
        crop = output / 'crop.png'
        run('screenshot', '--output', str(crop), '--overwrite', '--region-x', '50',
            '--region-y', '50', '--region-width', '120', '--region-height', '80')
        data = crop.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n', 'Expected PNG'
        assert struct.unpack('>II', data[16:24]) == (120, 80), 'Unexpected crop size'
        run('click', '--selector', '#scroll', '--timeout-ms', '5000')
        run('wait', 'text', '--selector', '#evidence', '--text', 'scroll-observed',
            '--timeout-ms', '5000')
        scrolled_crop = output / 'scrolled-crop.png'
        run('screenshot', '--output', str(scrolled_crop), '--overwrite', '--crop-x', '50',
            '--crop-y', '50', '--crop-width', '120', '--crop-height', '80')
        assert scrolled_crop.read_bytes() == data, 'Viewport crop changed after scrolling'
        evidence = {'drag_elapsed_seconds': elapsed, 'expected_markers': sorted(expected),
                    'crop_dimensions': [120, 80], 'results': results}
        (output / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
        print(f'Pointer smoke passed; evidence: {output / "evidence.json"}')
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
