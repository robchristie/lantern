#!/usr/bin/env python3
"""Ensure independent qualification truth cannot accept unrelated or synthetic input."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ADJUDICATOR = Path(__file__).with_name('adjudicate-comparison.py')


class AdjudicationContracts(unittest.TestCase):
    def setUp(self):
        self.form = [
            {'kind': 'input', 'case': 'form', 'inputEvents': [
                {'trusted': False, 'value': ''},
                {'trusted': True, 'inputType': 'insertText', 'value': 'Casey Example'}]},
            {'kind': 'submit', 'case': 'form', 'trusted': True, 'value': 'Casey Example'},
            {'kind': 'saved', 'case': 'form', 'saved': 'Casey Example'},
        ]
        self.canvas = [
            {'kind': 'frame', 'case': 'canvas', 'revision': 0, 'frame': 1,
             'centrePixel': [32, 95, 200, 255], 'pointerEvents': []},
            {'kind': 'frame', 'case': 'canvas', 'revision': 1, 'frame': 2,
             'centrePixel': [22, 128, 60, 255], 'pointerEvents': [
                 {'type': 'pointerdown', 'trusted': True, 'x': 160, 'y': 90},
                 {'type': 'click', 'trusted': True, 'x': 160, 'y': 90}]},
        ]

    def adjudicate(self, events):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / 'truth.jsonl').write_text(''.join(json.dumps({'generation': 1, 'event': e}) + '\n' for e in events))
            return json.loads(subprocess.check_output([sys.executable, str(ADJUDICATOR), str(path)], text=True))

    def test_normal_cli_preparation_does_not_invalidate_real_text(self):
        self.assertTrue(self.adjudicate(self.form)['form']['truth_pass'])

    def test_unrelated_trusted_event_cannot_validate_dom_assigned_value(self):
        self.form[0]['inputEvents'][1]['value'] = 'Unrelated text'
        self.assertFalse(self.adjudicate(self.form)['form']['truth_pass'])

    def test_untrusted_or_replayed_submission_is_not_one_real_submission(self):
        events = copy.deepcopy(self.form)
        events[1]['trusted'] = False
        self.assertFalse(self.adjudicate(events)['form']['truth_pass'])
        events = self.form + [copy.deepcopy(self.form[1])]
        self.assertFalse(self.adjudicate(events)['form']['truth_pass'])

    def test_rendered_change_requires_real_centre_input(self):
        self.assertTrue(self.adjudicate(self.canvas)['canvas']['truth_pass'])
        self.canvas[1]['pointerEvents'][1]['trusted'] = False
        self.assertFalse(self.adjudicate(self.canvas)['canvas']['truth_pass'])
        self.canvas[1]['pointerEvents'][1]['trusted'] = True
        self.canvas[1]['pointerEvents'][0]['x'] = 20
        self.assertFalse(self.adjudicate(self.canvas)['canvas']['truth_pass'])


if __name__ == '__main__':
    unittest.main()
