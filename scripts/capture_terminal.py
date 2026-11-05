#!/usr/bin/env python3
"""Capture the real running TUI via PTY and render its terminal cells, without invented UI.
Developer-only dependencies: Pillow, pyte. The app itself does not depend on Python.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import signal
import sqlite3
import struct
import subprocess
import time

import pyte
from PIL import Image, ImageDraw, ImageFont

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', default='target/release/aether')
parser.add_argument('--workspace', required=True)
parser.add_argument('--config', required=True)
parser.add_argument('--data-dir', required=True)
parser.add_argument('--session', required=True)
parser.add_argument('--output', default='docs/assets')
parser.add_argument('--font')
args = parser.parse_args()
columns, rows = 136, 42
screen = pyte.Screen(columns, rows)
stream = pyte.ByteStream(screen)
font_file = args.font or subprocess.check_output(['fc-match', 'JetBrains Mono', '-f', '%{file}'], text=True)
font = ImageFont.truetype(font_file, 16)
cell_w = round(font.getlength('M'))
cell_h = 23
output = Path(args.output)
output.mkdir(parents=True, exist_ok=True)
command = [str(Path(args.binary).resolve()), '--workspace', str(Path(args.workspace).resolve()),
           '--config', str(Path(args.config).resolve()), '--session', args.session]
env = dict(os.environ, TERM='xterm-256color', AETHER_DATA_DIR=str(Path(args.data_dir).resolve()))
# Presentation captures opt into terminal color; normal application NO_COLOR behavior is unchanged.
env.pop('NO_COLOR', None)
env['COLORTERM'] = 'truecolor'
pid, fd = pty.fork()
if pid == 0:
    os.execve(command[0], command, env)
fcntl.ioctl(fd, 0x5414, struct.pack('HHHH', rows, columns, 0, 0))
os.kill(pid, signal.SIGWINCH)

def pump(seconds=.3):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if select.select([fd], [], [], min(.05, max(0, end-time.monotonic())))[0]:
            try:
                data = os.read(fd, 65536)
            except OSError:
                return
            stream.feed(data)
            if b'\x1b[6n' in data:
                os.write(fd, b'\x1b[1;1R')

colors = {'default': '#cfdcf0', 'black': '#0c121e', 'red': '#f7768e', 'green': '#9ece6a',
          'brown': '#e0af68', 'yellow': '#e0af68', 'blue': '#7aa2f7', 'magenta': '#bb9af7',
          'cyan': '#7dcfff', 'white': '#cfdcf0', 'brightblack': '#7284a2'}
def color(value, background=False):
    if value == 'default' and background:
        return '#0c121e'
    return colors.get(value, '#'+value if len(value) == 6 else '#cfdcf0')

captures = []
def capture(name, title):
    width, height = columns*cell_w+64, rows*cell_h+90
    image = Image.new('RGB', (width, height), '#080d17')
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((12, 12, width-12, height-12), radius=18, fill='#0c121e', outline='#28374e', width=2)
    draw.rectangle((14, 52, width-14, 54), fill='#28374e')
    for i, fill in enumerate(['#f7768e', '#e0af68', '#9ece6a']):
        draw.ellipse((30+i*20, 28, 40+i*20, 38), fill=fill)
    draw.text((width//2, 33), title, font=font, fill='#95a9ca', anchor='mm')
    for y in range(rows):
        for x in range(columns):
            c = screen.buffer[y][x]
            fg, bg = color(c.fg), color(c.bg, True)
            if c.reverse:
                fg, bg = bg, fg
            left, top = 32+x*cell_w, 68+y*cell_h
            draw.rectangle((left, top, left+cell_w, top+cell_h), fill=bg)
            if c.data.strip():
                draw.text((left, top), c.data, font=font, fill=fg, stroke_width=0)
    path = output/name
    image.save(path)
    captures.append({'file': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                     'terminal_text': screen.display, 'columns': columns, 'rows': rows,
                     'source': 'actual executable, persisted real local-model session; PTY cells rendered with pyte/Pillow; presentation frame added'})

try:
    pump(1.0)
    assert any('AETHER' in line for line in screen.display), 'TUI did not render'
    capture('session-overview.png', 'AETHER / local multi-agent investigation')
    with sqlite3.connect(Path(args.data_dir)/'sessions.db') as database:
        records = [json.loads(row[0]) for row in database.execute('select record from agent_runs where root_session_id=? order by rowid', (args.session,))]
    roots = [r for r in records if r['parent_id'] is None]
    ordered = []
    def append(parent):
        for record in records:
            if record['parent_id'] == parent:
                ordered.append(record)
                append(record['id'])
    append(None)
    child = next(r for r in ordered if r['parent_id'] is not None and r['status']=='completed')
    index = ordered.index(child)
    os.write(fd, b'\x1bOQ')  # F2: focus the real tree
    pump(.15)
    os.write(fd, b'\x1b[B'*index)
    pump(.1)
    os.write(fd, b'\r')
    pump(.6)
    assert any('Agent detail' in line for line in screen.display), 'agent navigation did not open detail'
    capture('agent-inspector.png', 'AETHER / persisted agent inspector')
    captures[-1]['agent_id'] = child['id']
    os.write(fd, b'\x11')
    for _ in range(60):
        pump(.05)
        found, status = os.waitpid(pid, os.WNOHANG)
        if found:
            assert os.waitstatus_to_exitcode(status)==0
            pid=None
            break
finally:
    if pid:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    os.close(fd)
Path('docs/validation/screenshots.json').write_text(json.dumps({'session_id': args.session, 'captures': captures}, indent=2, ensure_ascii=False)+'\n')
print(json.dumps({'files': [c['file'] for c in captures], 'source': 'real TUI terminal captures'}, indent=2))
