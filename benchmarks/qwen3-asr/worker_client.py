"""Single-flight local subprocess supervisor; kill/restart on timeout/protocol failure.
No microphone, cloud, database or application provider integration.
"""
import json,os,selectors,subprocess,time,resource,threading
class Worker:
 def __init__(self,binary,model,threads=6,vad_only=False,timeout=120,crop_context=False,aligner_dir=None):
  self.binary=str(binary);self.model=str(model);self.threads=threads;self.vad_only=vad_only;self.timeout=timeout;self.crop_context=crop_context;self.aligner_dir=str(aligner_dir) if aligner_dir is not None else None;self.process=None;self.buffer=b'';self.generation=0;self.lock=threading.Lock()
 def start(self):
  if self.process is not None:raise RuntimeError('Worker already started')
  env=os.environ.copy();env.update(OPENBLAS_NUM_THREADS=str(self.threads),OMP_NUM_THREADS=str(self.threads))
  def limit():resource.setrlimit(resource.RLIMIT_AS,(18*1024**3,18*1024**3))
  args=[self.binary,self.model,str(self.threads)]+(['--vad-only'] if self.vad_only else [])+(['--crop-context'] if self.crop_context else [])+(['--aligner-dir',self.aligner_dir] if self.aligner_dir else [])
  self.process=subprocess.Popen(args,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,env=env,preexec_fn=limit)
  self.buffer=b'';self.generation+=1
  try:
   ready=self.receive(self.timeout)
   if ready.get('event')!='ready' or ready.get('protocol')!=1:raise RuntimeError('Invalid handshake')
   return ready
  except BaseException:self.cancel();raise
 def receive(self,timeout):
  deadline=time.monotonic()+timeout
  with selectors.DefaultSelector() as selector:
   selector.register(self.process.stdout,selectors.EVENT_READ)
   while b'\n' not in self.buffer:
    remaining=deadline-time.monotonic()
    if remaining<=0 or not selector.select(remaining):raise TimeoutError('Worker deadline exceeded')
    data=os.read(self.process.stdout.fileno(),8192)
    if not data:raise RuntimeError('Worker exited or truncated its response')
    self.buffer+=data
    if len(self.buffer)>128*1024:raise RuntimeError('Worker response too large')
  line,self.buffer=self.buffer.split(b'\n',1)
  return json.loads(line)
 def request(self,request,timeout=None):
  if not self.lock.acquire(blocking=False):raise RuntimeError('Only one in-flight request permitted')
  try:return self._request(request,timeout)
  finally:self.lock.release()
 def _request(self,request,timeout=None):
  if self.process is None:self.start()
  line=json.dumps(request,ensure_ascii=False).encode()+b'\n'
  if len(line)>4096:raise ValueError('Request too large')
  try:
   self.process.stdin.write(line);self.process.stdin.flush()
   response=self.receive(self.timeout if timeout is None else timeout)
   if response.get('id')!=request['id']:raise RuntimeError('Response ID mismatch')
   return response
  except BaseException:self.cancel();raise
 def cancel(self):
  p=self.process;self.process=None;self.buffer=b''
  if p is not None:
   if p.poll() is None:p.kill()
   p.wait(timeout=10)
   p.stdin.close();p.stdout.close()
 def __enter__(self):self.start();return self
 def __exit__(self,*args):self.cancel()
