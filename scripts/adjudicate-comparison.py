#!/usr/bin/env python3
"""Summarise independent fixture truth and recorded CLI costs, not agent claims."""
import argparse
from datetime import datetime
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('run_dir', type=Path)
args = parser.parse_args()


def records(name):
    path = args.run_dir / name
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


truth = records('truth.jsonl')
observer = records('observer.jsonl')
commands = records('calls/commands.jsonl')
lifecycle = records('lifecycle.jsonl')


def events(case, kind=None, generation=None):
    return [r['event'] for r in truth if r['event']['case'] == case
            and (kind is None or r['event']['kind'] == kind)
            and (generation is None or r['generation'] == generation)]


def real_input(case):
    inputs = events(case, 'input')
    return any(i['trusted'] and i.get('inputType') == 'insertText' for e in inputs for i in e['inputEvents'])


def trusted_text(case, value, generation=None):
    return any(i['trusted'] and i.get('inputType') == 'insertText' and i.get('value') == value
               for e in events(case, 'input', generation) for i in e['inputEvents'])


def trusted_submit(case, value, generation=None):
    submissions = events(case, 'submit', generation)
    return len(submissions) == 1 and submissions[0].get('trusted') is True and submissions[0]['value'] == value


form = events('form', 'saved')
failure = events('failure', 'failure')
canvas = events('canvas', 'frame')
recovery_before = events('recovery', 'loaded', 1)
recovery_after = events('recovery', 'loaded', 2)
summary = {
    'form': {
        'submit_count': len(events('form', 'submit')),
        'trusted_input': real_input('form'),
        'trusted_text_values': sorted({i['value'] for e in events('form', 'input') for i in e['inputEvents'] if i['trusted'] and 'value' in i}),
        'submit_events': events('form', 'submit'),
        'saved_names': [e['saved'] for e in form],
        'truth_pass': trusted_submit('form', 'Casey Example') and trusted_text('form', 'Casey Example') and len(form) == 1 and form[0]['saved'] == 'Casey Example',
    },
    'failure': {
        'submit_count': len(events('failure', 'submit')),
        'trusted_input': real_input('failure'),
        'trusted_text_values': sorted({i['value'] for e in events('failure', 'input') for i in e['inputEvents'] if i['trusted'] and 'value' in i}),
        'submit_events': events('failure', 'submit'),
        'saved_count': len(events('failure', 'saved')),
        'trusted_intended_text_and_submit': trusted_text('failure', 'Casey Example') and trusted_submit('failure', 'Casey Example'),
        'http_statuses': [e['httpStatus'] for e in failure],
        'runtime_messages': [e['message'] for e in events('failure', 'runtime-error')],
        'independent_http': [e for e in observer if e['kind'] == 'http-error'],
        'independent_runtime': [e for e in observer if e['kind'] == 'pageerror'],
    },
    'layout': {'loaded': bool(events('layout', 'loaded')), 'visual_adjudication': 'Requires opening agent PNG and comparing its report'},
    'canvas': {
        'frames': [{'revision': e['revision'], 'frame': e['frame'], 'pixel': e['centrePixel'], 'pointers': e['pointerEvents']} for e in canvas],
        'truth_pass': len(canvas) == 2 and canvas[0]['revision'] == 0 and canvas[0]['centrePixel'] == [32, 95, 200, 255]
        and canvas[1]['revision'] == 1 and canvas[1]['frame'] == 2 and canvas[1]['centrePixel'] == [22, 128, 60, 255]
        and [p['type'] for p in canvas[1]['pointerEvents']] == ['pointerdown', 'click'] and all(p['trusted'] and abs(p['x'] - 160) <= 1 and abs(p['y'] - 90) <= 1 for p in canvas[1]['pointerEvents']),
        'visual_adjudication': 'Requires opening agent before/after PNGs',
    },
    'recovery': {
        'before_restart_loaded_names': [e['saved'] for e in recovery_before],
        'after_restart_loaded_names': [e['saved'] for e in recovery_after],
        'before_restart_saved_names': [e['saved'] for e in events('recovery', 'saved', 1)],
        'after_restart_saved_names': [e['saved'] for e in events('recovery', 'saved', 2)],
        'restarts': [e for e in lifecycle if e['kind'] == 'restarted'],
        'trusted_intended_text_and_submits': trusted_text('recovery', 'Morgan Draft', 1) and trusted_text('recovery', 'Morgan Recovered', 2) and trusted_submit('recovery', 'Morgan Draft', 1) and trusted_submit('recovery', 'Morgan Recovered', 2),
    },
    'metrics': {
        'cli_invocations': len(commands),
        'cli_nonzero_exits': sum(c['exit_code'] != 0 for c in commands),
        'cli_elapsed_sum_seconds': sum(c['elapsed_seconds'] for c in commands),
        'cli_window_seconds': (max(datetime.fromisoformat(c['finished_utc']) for c in commands) - min(datetime.fromisoformat(c['started_utc']) for c in commands)).total_seconds() if commands else None,
        'model_context_tokens': None, 'orchestration_tool_calls': None,
    },
    'initial_identities': [e for e in lifecycle if e['kind'] == 'started'],
    'limitations': ['Independent fixture state is not agent verification.', 'False passes and completion require comparing the agent report to this truth.', 'Elapsed CLI window excludes initial skill reading and final report writing.'],
}
print(json.dumps(summary, indent=2))
