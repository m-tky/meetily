"""Pure attribution/report checks. No models, downloads, or capture."""
import unittest
from evaluate_forced_alignment import assign,characterize,groups,edit_distance,turn_metrics


def unit(text,a,b,flags=None):return dict(text=text,start_seconds=a,end_seconds=b,flags=flags or [])
def seg(who,a,b):return dict(speaker=who,start=a,end=b)


class AlignmentTests(unittest.TestCase):
    def test_character_turn_splits_and_preserves_text(self):
        a=assign([unit('車',0,1),unit('本',1,2)],[seg(0,0,1),seg(1,1,2)])
        g=groups(a)
        self.assertEqual([x['speaker'] for x in g],[0,1])
        self.assertEqual(''.join(x['text'] for x in g),'車本')
        self.assertTrue(all(x['confirmed'] is False for x in g))

    def test_overlap_and_boundary_not_arbitrary_winner(self):
        a=assign([unit('本',0,1)],[seg(0,0,.9),seg(1,.8,1)])
        self.assertIsNone(a[0]['speaker'])
        self.assertEqual(a[0]['attribution_reason'],'multiple_speakers')

    def test_gap_zero_and_punctuation_kept_unknown(self):
        a=assign([unit('車',0,1),unit('本',1,1,['zero_duration']),unit('。',1,2,['non_spoken_symbol'])],[seg(0,0,.5)])
        self.assertTrue(all(x['speaker'] is None for x in a))
        self.assertEqual(''.join(x['text'] for x in groups(a)),'車本。')

    def test_same_speaker_duplicate_segments_union_not_double_count(self):
        a=assign([unit('車',0,1)],[seg(0,0,.6),seg(0,.4,.8)])
        self.assertEqual(a[0]['speaker'],0)

    def test_speaker_error_counted_even_when_text_is_correct(self):
        f=dict(suite='speaker_turns',placements=[dict(start=0,end=1,speaker=0,text='車'),dict(start=1,end=2,speaker=1,text='本')])
        r=dict(diar=dict(segments=[seg(5,0,1),seg(6,1,2)]),response=dict(text='車本'),
               assigned_units=[dict(text='車',speaker=5),dict(text='本',speaker=5)],oracle_alignment=None)
        m=turn_metrics(f,r)
        self.assertEqual(m['normalized_alnum_char_errors'],0)
        self.assertEqual(m['source_attributed_char_errors_proxy'],1)
        self.assertEqual(edit_distance('車本','車山本'),1)

    def test_official_word_inherits_one_label_but_counts_all_characters(self):
        f=dict(suite='speaker_turns',placements=[dict(start=0,end=1,speaker=0,text='車車'),dict(start=1,end=2,speaker=1,text='本')])
        r=dict(diar=dict(segments=[seg(5,0,1),seg(6,1,2)]),response=dict(text='車車本'),
               assigned_units=[dict(text='車車',speaker=5),dict(text='本',speaker=6)],oracle_alignment=None)
        m=turn_metrics(f,r)
        self.assertEqual(m['recognized_chars'],3)
        self.assertEqual(m['source_attributed_char_errors_proxy'],0)

    def test_alignment_does_not_verify_recognition(self):
        r=characterize(dict(status='aligned',text='山。',units=[unit('山',0,0,['zero_duration']),unit('。',0,1,['non_spoken_symbol'])],confirmed=False))
        self.assertTrue(r['text_preserved']);self.assertEqual(r['zero_duration_units'],1)
        self.assertEqual(r['punctuation_units'],1)


if __name__=='__main__':unittest.main()
