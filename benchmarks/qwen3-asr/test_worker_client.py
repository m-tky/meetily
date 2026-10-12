import pathlib,tempfile,unittest,sys
from worker_client import Worker
class Tests(unittest.TestCase):
 def mock(self,folder,behavior):
  p=pathlib.Path(folder)/'worker';p.write_text(f'''#!{sys.executable}
import sys,json,time
print(json.dumps({{"event":"ready","protocol":1}}),flush=True)
for line in sys.stdin:
 r=json.loads(line)
 {behavior}
''');p.chmod(0o755);return p
 def test_timeout_kills_and_restart(self):
  with tempfile.TemporaryDirectory() as d:
   p=self.mock(d,'time.sleep(5)');w=Worker(p,'unused',timeout=1);w.start()
   with self.assertRaises(TimeoutError):w.request({'op':'ping','id':'x'},timeout=.1)
   self.assertIsNone(w.process)
   self.mock(d,'print(json.dumps({"id":r["id"],"status":"pong"}),flush=True)')
   self.assertEqual(w.request({'op':'ping','id':'y'})['status'],'pong');self.assertEqual(w.generation,2);w.cancel()
 def test_id_mismatch_fails_closed(self):
  with tempfile.TemporaryDirectory() as d:
   w=Worker(self.mock(d,'print(json.dumps({"id":"wrong"}),flush=True)'),'unused',timeout=1)
   with self.assertRaisesRegex(RuntimeError,'ID mismatch'):w.request({'op':'ping','id':'x'})
   self.assertIsNone(w.process)
 def test_busy_rejects_not_queue(self):
  w=Worker('unused','unused');w.lock.acquire()
  with self.assertRaisesRegex(RuntimeError,'one in-flight'):w.request({'id':'x'})
  w.lock.release()
 def test_excessive_response(self):
  with tempfile.TemporaryDirectory() as d:
   w=Worker(self.mock(d,'print("x"*140000,flush=True)'),'unused',timeout=1)
   with self.assertRaisesRegex(RuntimeError,'too large'):w.request({'op':'ping','id':'x'})
   self.assertIsNone(w.process)
if __name__=='__main__':unittest.main()
