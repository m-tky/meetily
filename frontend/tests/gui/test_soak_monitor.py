import json
import pathlib
import tempfile
import unittest
from soak_monitor import SoakMonitor, status_rss


class MonitorTests(unittest.TestCase):
    def test_rss_parser(self):
        self.assertEqual(status_rss('Name:\ttest\nVmRSS:\t123 kB\n'),123)
        self.assertEqual(status_rss('Name:\ttest\n'),0)

    def test_plateau_summary_excludes_load(self):
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp)
            monitor=SoakMonitor(root,123,root/'native',root/'worker')
            monitor.worker_pids={456}
            monitor.rows=[{'seconds':sec,'native_rss_kib':native,'worker_rss_kib':worker,
                'temporary_inputs':pending,'saved_segments':segments}
                for sec,native,worker,pending,segments in [(0,10,999,0,0),(65,20,100,1,2),(90,22,102,0,3),(550,24,104,1,40),(600,26,106,1,44)]]
            monitor.finish(600,44)
            report=json.loads((root/'soak-summary.json').read_text())
            self.assertEqual(report['worker_rss_kib']['peak'],999)
            self.assertEqual(report['worker_rss_kib']['head_60_120s_median'],101)
            self.assertEqual(report['worker_rss_kib']['last_60s_median'],105)
            self.assertEqual(report['worker_pids'],[456])
            self.assertTrue(report['not_a_leak_or_long_meeting_acceptance'])

    def test_nix_wrapper_companion(self):
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp)
            (root/'bin').mkdir();(root/'libexec').mkdir()
            wrapper=root/'bin/meetily';wrapper.write_text('wrapper')
            binary=root/'libexec/meetily';binary.write_text('synthetic placeholder')
            monitor=SoakMonitor(root,123,wrapper,root/'worker')
            self.assertEqual(monitor.native_executables,{wrapper,binary})

    def test_missing_native_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp)
            monitor=SoakMonitor(root,123,root/'native',root/'worker')
            monitor.rows=[{'native_rss_kib':0}]
            with self.assertRaises(AssertionError): monitor.finish(600,44)


if __name__=='__main__': unittest.main()
