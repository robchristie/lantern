#!/usr/bin/env python3
"""Verify rendered graphics status and actual red/green canvas pixels.

The caller supplies a trusted graphics-smoke.html URL, selected disposable page,
and browser lifecycle. Serve the WebGPU fixture on container-local localhost
(or an approved secure origin); this runner never changes browser security flags.
"""
import argparse
import json
from pathlib import Path
import struct
import subprocess
import zlib


def pixels(path):
    """Decode Chromium's non-interlaced 8-bit RGB/RGBA PNG screenshot."""
    data = path.read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    offset, compressed = 8, bytearray()
    while offset < len(data):
        length = struct.unpack('>I', data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        chunk = data[offset + 8:offset + 8 + length]
        if kind == b'IHDR':
            width, height, depth, colour, compression, filtering, interlace = struct.unpack('>IIBBBBB', chunk)
        elif kind == b'IDAT':
            compressed.extend(chunk)
        offset += 12 + length
    assert (width, height, depth, interlace) == (256, 256, 8, 0)
    assert colour in (2, 6)
    channels = 3 if colour == 2 else 4
    stride = width * channels
    raw = zlib.decompress(compressed)
    rows, previous = [], bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        method, row = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        assert method in range(5)
        for x in range(stride):
            left = row[x - channels] if x >= channels else 0
            above = previous[x]
            upper_left = previous[x - channels] if x >= channels else 0
            p = left + above - upper_left
            choices = (left, above, upper_left)
            paeth = min(choices, key=lambda value: abs(p - value))
            predictor = (0, left, above, (left + above) // 2, paeth)[method]
            row[x] = (row[x] + predictor) % 256
        rows.append(row)
        previous = row
    return [list(rows[y][x * channels:x * channels + 3]) for x, y in [(8, 8), (128, 128)]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lantern', default='target/debug/lantern')
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--target-id', required=True)
    parser.add_argument('--fixture-url', required=True)
    parser.add_argument('--api', choices=['webgl', 'webgpu'], required=True)
    parser.add_argument('--output-dir', default='.smoogle/artifacts/graphics-smoke')
    args = parser.parse_args()
    output = Path(args.output_dir)
    output.mkdir(parents=True, exist_ok=True)
    results = []

    def run(*command):
        result = subprocess.run([args.lantern, *command, '--endpoint', args.endpoint,
                                 '--target-id', args.target_id, '--json'],
                                check=True, capture_output=True, text=True, timeout=45)
        value = json.loads(result.stdout)
        assert value.get('ok'), value
        results.append(value)
        return value

    separator = '&' if '?' in args.fixture_url else '?'
    run('open', args.fixture_url + separator + 'api=' + args.api)
    wait = run('wait', 'text', '--selector', '#status', '--text', '"status":"rendered"',
               '--timeout-ms', '15000')
    assert wait['wait']['matched'], wait
    run('dom', '--depth', '6', '--max-nodes', '40')
    screenshot = output / f'{args.api}.png'
    run('screenshot', '--output', str(screenshot), '--overwrite', '--region-x', '0',
        '--region-y', '0', '--region-width', '256', '--region-height', '256')
    observed = pixels(screenshot)
    assert observed[0][0] > 200 and max(observed[0][1:]) < 40, observed
    assert observed[1][1] > 200 and max(observed[1][::2]) < 40, observed
    evidence = {'api': args.api, 'sample_pixels': observed, 'results': results}
    path = output / f'{args.api}-evidence.json'
    path.write_text(json.dumps(evidence, indent=2) + '\n')
    print(f'Graphics smoke passed; evidence: {path}')


if __name__ == '__main__':
    main()
