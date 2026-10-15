"""Pure range/attribution contract checks; no models or capture."""
import unittest
from attach_diarization_labels import annotate


def seg(who,a,b):return dict(speaker=who,start=a,end=b)
def gate(a,b):return dict(start_sample=a,end_sample=b)


class AttributionTests(unittest.TestCase):
    def test_single_speaker_is_still_unconfirmed(self):
        r=annotate(0,16000,[seg(2,0,1)],[gate(0,16000)])
        self.assertEqual(r['single_speaker_id'],2)
        self.assertFalse(r['confirmed'])
        self.assertTrue(r['provisional'])

    def test_multiple_turns_not_arbitrary_text_winner(self):
        r=annotate(0,16000,[seg(0,0,.75),seg(1,.75,1)],[gate(0,16000)])
        self.assertEqual(r['speaker_ids'],[0,1])
        self.assertIsNone(r['single_speaker_id'])
        self.assertEqual(r['overlap_voiced_samples'],0)

    def test_overlap_not_claimed_separated_audio(self):
        r=annotate(0,16000,[seg(0,0,1),seg(1,.5,1)],[gate(0,16000)])
        self.assertEqual(r['overlap_voiced_samples'],8000)
        self.assertIsNone(r['single_speaker_id'])

    def test_unknown_speech_retained_no_false_speaker(self):
        r=annotate(0,16000,[seg(0,.5,1)],[gate(0,16000)])
        self.assertEqual(r['unknown_voiced_samples'],8000)
        self.assertEqual(r['voiced_samples'],16000)
        self.assertIsNone(r['single_speaker_id'])

    def test_absolute_origin_and_gate_union(self):
        r=annotate(160000,176000,[seg(3,10,11)],[gate(0,12000),gate(8000,16000)])
        self.assertEqual(r['voiced_samples'],16000)
        self.assertEqual(r['label_spans'][0]['start_sample'],160000)
        self.assertEqual(r['single_speaker_id'],3)

    def test_no_gate_does_not_invent_voice(self):
        r=annotate(0,16000,[seg(0,0,1)],[])
        self.assertEqual(r['speaker_ids'],[])
        self.assertEqual(r['voiced_samples'],0)

    def test_invalid_bounds_rejected(self):
        for labels,gates in (([seg(0,float('nan'),1)],[gate(0,16000)]),
                             ([seg(0,1,0)],[]),([seg(8,0,1)],[]),
                             ([],[gate(-1,16000)]),([],[gate(0,16001)])):
            with self.assertRaises(ValueError):annotate(0,16000,labels,gates)


if __name__=='__main__':unittest.main()
