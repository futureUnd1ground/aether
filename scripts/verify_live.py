#!/usr/bin/env python3
"""Owner-attended real-model coding check. No scripted provider or automatic approval."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--config', required=True, type=Path)
parser.add_argument('--binary', default='target/release/aether', type=Path)
parser.add_argument('--resume-summary', action='store_true')
parser.add_argument('--output', default='live-validation.json', type=Path)
args = parser.parse_args()
if not sys.stdin.isatty():
    parser.error('run in an interactive terminal to review file/shell approvals')
binary, config = args.binary.resolve(), args.config.resolve(strict=True)
with tempfile.TemporaryDirectory(prefix='aether-live-coding-') as directory:
    root = Path(directory)
    (root / 'answer.py').write_text('def add(a, b):\n    return a - b\n')
    tests = ('import unittest\nfrom answer import add\n'
             'class Addition(unittest.TestCase):\n'
             '    def test_add(self):\n'
             '        self.assertEqual(add(2, 3), 5)\n'
             '        self.assertEqual(add(-2, 2), 0)\n')
    (root / 'test_answer.py').write_text(tests)
    test_command = [sys.executable, '-m', 'unittest', '-v']
    before = subprocess.run(test_command, cwd=root, capture_output=True, text=True)
    assert before.returncode != 0, 'fixture must fail initially'
    env = dict(os.environ, AETHER_DATA_DIR=str(root / 'data'))
    base = [str(binary), '--workspace', str(root), '--config', str(config)]
    prompt = ('Read answer.py and test_answer.py using those relative paths. '
              'Fix add using the read_file sha256 in write_file. Do not change tests. '
              'Run python3 -m unittest -v using shell and report the actual outcome.')
    result = subprocess.run(base + ['--mode', 'build', 'run', '--allow-read', prompt], env=env)
    after = subprocess.run(test_command, cwd=root, capture_output=True, text=True)
    sessions = json.loads(subprocess.check_output(base + ['sessions', '--json'], env=env, text=True))
    session_id = sessions[0]['id']
    resume = None
    if args.resume_summary and result.returncode != 0:
        resume = subprocess.run(base + ['--session', session_id, '--mode', 'ask', 'run', '--json',
                                       'Summarize the already recorded tool outcomes. Do not call tools.'],
                                env=env, capture_output=True, text=True)
    report = {
        'agent_exit': result.returncode,
        'before_test_exit': before.returncode,
        'after_test_exit': after.returncode,
        'before_test_output': before.stdout + before.stderr,
        'after_test_output': after.stdout + after.stderr,
        'tests_unchanged': (root / 'test_answer.py').read_text() == tests,
        'resulting_file': (root / 'answer.py').read_text(),
        'resume_exit': resume.returncode if resume else None,
        'resume_output': resume.stdout if resume else None,
        'session': json.loads(subprocess.check_output(base + ['export', session_id], env=env, text=True)),
    }
args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
print(f'Evidence: {args.output.resolve()}')
# Follow-up success does not hide a failure in the original coding turn.
sys.exit(0 if result.returncode == 0 and after.returncode == 0 and report['tests_unchanged'] else 1)
