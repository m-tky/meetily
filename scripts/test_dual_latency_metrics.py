import json
import unittest

from dual_latency_metrics import summarize_transport


class TransportMetricsTests(unittest.TestCase):
    def test_source_identity_counts_gaps_queue_and_late_are_independent(self):
        log = '''
Audio transport handoff: source=Microphone, chunk=0, samples=480, at_s=1.900000
Audio transport handoff: source=System, chunk=0, samples=4800, at_s=0.600000
Audio transport handoff: source=Microphone, chunk=1, samples=960, at_s=1.950000
Audio transport receive: source=Microphone, chunk=0, samples=480, at_s=1.903000, handoff_s=1.900000
Discarded 17 late audio samples from an already emitted interval (source=Microphone)
Discarded 9 late audio samples from an already emitted interval (source=System)
Discarded 3 late audio samples from an already emitted interval (source=Microphone)
Audio transport mix: start_sample=0, samples=28800, at_s=2.400000, finishing=false, degraded_started=true
Audio transport mix: start_sample=28800, samples=100, at_s=2.500000, finishing=true, degraded_started=false
'''
        result = summarize_transport(log)
        mic = result['transport_timing']['Microphone']
        self.assertEqual(mic['samples'], 1440)
        self.assertEqual(mic['callbacks'], 2)
        self.assertAlmostEqual(mic['gap_max_ms'], 50)
        self.assertAlmostEqual(mic['queue_lag_max_ms'], 3)
        self.assertEqual(result['transport_timing']['System']['first_handoff_s'], .6)
        self.assertEqual(result['late_samples_by_source'], {'Microphone': 20, 'System': 9})
        self.assertEqual(result['degraded_mix_times_s'], [2.4])
        self.assertEqual(result['first_mix_output_s'], 2.4)

    def test_empty_log_does_not_invent_ready_or_output(self):
        result = summarize_transport('')
        self.assertIsNone(result['first_mix_output_s'])
        self.assertEqual(result['transport_timing']['Microphone']['callbacks'], 0)
        self.assertIsNone(result['transport_timing']['System']['first_handoff_s'])

    def test_unrelated_contents_are_never_exported(self):
        secret = 'LOCAL_CONTENT_NOT_FOR_EXPORT'
        result = summarize_transport('transcript-update: ' + secret + '\n' + secret)
        self.assertNotIn(secret, json.dumps(result))
        self.assertEqual(result, summarize_transport(''))

    def test_legacy_unknown_discard_is_not_assigned_to_a_source(self):
        result = summarize_transport('Discarded 900 late audio samples from an already emitted interval')
        self.assertEqual(sum(result['late_samples_by_source'].values()), 0)


if __name__ == '__main__':
    unittest.main()
