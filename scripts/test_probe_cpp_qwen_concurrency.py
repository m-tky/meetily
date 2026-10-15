"""Pure concurrency-report/schedule checks; no models or audio devices."""
import unittest
from probe_cpp_qwen_concurrency import request_metrics, schedule


class ConcurrencyTests(unittest.TestCase):
    def test_frozen_schedule_exact_coverage(self):
        self.assertEqual(schedule(),[(0,10),(10,20),(20,30),(30,40),(40,50),(50,60)])
        for args in ((120,10),(60,5),(0,10)):
            with self.assertRaises(ValueError):schedule(*args)

    def test_queue_wait_and_stop_tail_not_compute_rtf(self):
        e=[dict(request_seconds=4,queue_wait_seconds=0,completed_seconds=14,
                response=dict(asr_calls=1,text='public fixture',status='recognized')),
           dict(request_seconds=8,queue_wait_seconds=3,completed_seconds=71,
                response=dict(asr_calls=1,text='',status='error'))]
        r=request_metrics(e,60)
        self.assertEqual(r['request_rtf'],.2)
        self.assertEqual(r['maximum_queue_wait_seconds'],3)
        self.assertEqual(r['stop_drain_seconds'],11)
        self.assertEqual(r['errors'],1)
        self.assertEqual(r['nonempty_responses'],1)

    def test_missing_events_rejected(self):
        with self.assertRaises(ValueError):request_metrics([],60)


if __name__=='__main__':unittest.main()
