import copy
import unittest

from evaluate_robustness import rms
from validate_current_vad import LEXICAL, freeze_plan, fresh_tones, validate_manifest


class CurrentValidationTests(unittest.TestCase):
    def fixture(self):
        rows = []
        for suite, count in [('fleurs-other26', 208), ('new-lexical', 12), ('new-negatives', 34)]:
            for i in range(count):
                rows.append(dict(id=f'{suite}-{i}', path=f'/public/{suite}-{i}.wav', suite=suite,
                                 reference='' if suite == 'new-negatives' else 'speech',
                                 purpose='silence' if suite == 'new-negatives' else 'accuracy',
                                 audio_seconds=1.0))
        return dict(rows=rows, source_records=[dict(title=t, published_reference=r, speaker=s)
                                               for t, r, s in LEXICAL])

    def test_frozen_candidate_no_retuning_or_source_holdout_claim(self):
        plan = freeze_plan()
        self.assertEqual(plan['candidate']['positive'], .7)
        self.assertEqual(plan['candidate']['gain_policy'], 'peak20')
        self.assertEqual(plan['candidate']['minimum_ms_strictly_greater'], 90)
        self.assertFalse(plan['retuning'])
        self.assertFalse(plan['source_holdout'])
        self.assertFalse(plan['conversational_acknowledgements'])

    def test_manifest_fail_closed(self):
        manifest = self.fixture()
        validate_manifest(manifest)
        missing = copy.deepcopy(manifest)
        missing['rows'].pop()
        with self.assertRaises(ValueError):
            validate_manifest(missing)
        duplicate = copy.deepcopy(manifest)
        duplicate['rows'][1]['id'] = duplicate['rows'][0]['id']
        with self.assertRaises(ValueError):
            validate_manifest(duplicate)
        reference = copy.deepcopy(manifest)
        reference['rows'][0]['reference'] = ''
        with self.assertRaises(ValueError):
            validate_manifest(reference)
        duration = copy.deepcopy(manifest)
        duration['rows'][0]['audio_seconds'] = float('nan')
        with self.assertRaises(ValueError):
            validate_manifest(duration)
        identity = copy.deepcopy(manifest)
        identity['source_records'][0]['published_reference'] = 'はい'
        with self.assertRaises(ValueError):
            validate_manifest(identity)

    def test_new_tones_are_deterministic_distinct_and_unit_rms(self):
        first = fresh_tones(16000, (83, 166, 332))
        self.assertEqual(first, fresh_tones(16000, (83, 166, 332)))
        self.assertNotEqual(first, fresh_tones(16000, (149, 298, 596)))
        self.assertAlmostEqual(rms(first), 1.0, places=12)


if __name__ == '__main__':
    unittest.main()
