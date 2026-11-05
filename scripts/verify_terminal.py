#!/usr/bin/env python3
"""Linux/Unix PTY regression: real executable, terminal input, drafts and SIGKILL recovery."""
import argparse, fcntl, json, os, pty, re, select, signal, sqlite3, struct, tempfile, time, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

class Terminal:
    def __init__(self, command, env):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.execve(command[0], command, env)
        self.output = bytearray()
        self.exited = False
        self.resize(32, 120)
    def resize(self, rows, columns):
        fcntl.ioctl(self.fd, 0x5414, struct.pack('HHHH', rows, columns, 0, 0))
        os.kill(self.pid, signal.SIGWINCH)
    def send(self, data): os.write(self.fd, data)
    def visible(self): return re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]",b"",bytes(self.output))
    def pump(self, seconds=.15):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([self.fd], [], [], min(.05,max(0,end-time.monotonic())))[0]:
                try: data=os.read(self.fd, 65536)
                except OSError: break
                self.output.extend(data)
                if b'\x1b[6n' in data: self.send(b'\x1b[1;1R')
    def wait_for(self, condition, timeout=5):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            self.pump(.05)
            if condition(): return
        raise AssertionError('terminal condition timed out: '+repr(bytes(self.output[-1000:])))
    def close(self, crash=False):
        if self.exited: return
        if crash: os.kill(self.pid,signal.SIGKILL)
        else: self.send(b'\x11')  # Ctrl+Q
        end=time.monotonic()+5
        while time.monotonic()<end:
            self.pump(.05)
            pid,status=os.waitpid(self.pid,os.WNOHANG)
            if pid:
                self.exited=True
                os.close(self.fd)
                if not crash: assert os.waitstatus_to_exitcode(status)==0, status
                return
        os.kill(self.pid,signal.SIGKILL);os.waitpid(self.pid,0)
        self.exited=True;os.close(self.fd)
        raise AssertionError('graceful terminal shutdown timed out')

class SlowStream(BaseHTTPRequestHandler):
    started=threading.Event()
    def log_message(self,*args): pass
    def do_POST(self):
        self.rfile.read(int(self.headers['Content-Length']))
        self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()
        self.started.set()
        try:
            for i in range(40):
                self.wfile.write(('data: '+json.dumps({'choices':[{'delta':{'content':'STREAM_CHUNK '}}]})+'\n\n').encode())
                self.wfile.flush();time.sleep(.05)
            self.wfile.write(b'data: [DONE]\n\n');self.wfile.flush()
        except (BrokenPipeError,ConnectionResetError): pass

p=argparse.ArgumentParser();p.add_argument('--binary',default='target/release/aether');p.add_argument('--output',default='docs/terminal-validation.json');a=p.parse_args()
binary=str(Path(a.binary).resolve()); checks=[]
with tempfile.TemporaryDirectory(prefix='aether-pty-') as tmp:
    root=Path(tmp);config=root/'config.toml';config.write_text('[provider]\nmodel="terminal-fixture"\n')
    env=dict(os.environ,TERM='xterm-256color',AETHER_DATA_DIR=str(root/'data'))
    cmd=[binary,'--workspace',tmp,'--config',str(config)]
    term=Terminal(cmd,env)
    try:
        term.wait_for(lambda:b'AETHER' in term.output)
        checks.append('first frame rendered in PTY')
        term.send(b'\x1b[200~first line\nsecond line\x1b[201~');term.pump(1)
        db=root/'data/sessions.db'
        with sqlite3.connect(db) as c:
            sid,draft=c.execute('select id,draft from sessions').fetchone()
        assert draft=='first line\nsecond line',repr(draft)
        checks.append('bracketed multiline paste saved after debounce')
        term.send(b'\x08');term.pump(.1);term.send(b'\x1b');term.resize(12,40);term.pump(.2)
        term.close();checks.append('help, narrow resize and clean shutdown')
        term=Terminal(cmd+['--session',sid],env)
        term.wait_for(lambda:b'second' in term.visible())
        term.send(b'\x1b[200~RECOVERED\x1b[201~');term.pump(1);term.close(crash=True)
        with sqlite3.connect(db) as c:
            draft=c.execute('select draft from sessions where id=?',(sid,)).fetchone()[0]
            assert 'RECOVERED' in draft
            assert c.execute('pragma integrity_check').fetchone()[0]=='ok'
        checks.append('committed draft survives SIGKILL; SQLite integrity_check ok')
        term=Terminal(cmd+['--session',sid],env)
        term.wait_for(lambda:b'RECOVERED' in term.visible());term.close()
        checks.append('same session reopened after abrupt process death')
        server=ThreadingHTTPServer(('127.0.0.1',0),SlowStream)
        server.daemon_threads=True
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        config.write_text('[provider]\nmodel="terminal-fixture"\nbase_url="http://127.0.0.1:'+str(server.server_port)+'/v1"\n')
        term=Terminal(cmd,env)
        try:
            term.wait_for(lambda:b'AETHER' in term.output)
            term.send(b'stream-test\r')
            term.wait_for(lambda:SlowStream.started.is_set())
            term.wait_for(lambda:b'STREAM_CHUNK' in term.visible())
            term.send(b'\x1b[200~draft while streaming\x1b[201~');term.pump(.15)
            term.send(b'\x08');term.wait_for(lambda:b'Help' in term.visible())
            term.send(b'\x1b');term.pump(.05);term.send(b'\x1b');term.pump(.2);term.close()
            with sqlite3.connect(db) as c:
                assert c.execute('select draft from sessions order by rowid desc limit 1').fetchone()[0]=='draft while streaming'
            checks.append('input/help remain usable during network streaming; Esc + shutdown preserve new draft')
        finally:
            if not term.exited: term.close(crash=True)
            server.shutdown();server.server_close()

    finally:
        if not term.exited: term.close(crash=True)
result={'platform':os.uname().sysname,'binary':binary,'checks':checks,'result':'passed','limits':'PTY smoke/regression, not visual UX review or latency benchmark'}
Path(a.output).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
