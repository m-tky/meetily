import unittest
import numpy as np

from compare_vad_gain import POLICIES, freeze_plan, quantize, transform, validate_manifest
from reference_current_vad import compare, probability_map


class ReferenceGainTests(unittest.TestCase):
    def record(self, probabilities):
        return dict(file='/public.wav', policy='peak20', gain=20.0,
                    real_samples=2048, probabilities=probabilities)

    def test_parity_bit_equal_and_gate_mismatch(self):
        a = probability_map([self.record([.1, .8, .8, .8])])
        self.assertTrue(compare(a, a)['passed'])
        b = probability_map([self.record([.1, .6, .6, .6])])
        self.assertFalse(compare(a, b)['passed'])
        self.assertEqual(len(compare(a, b)['gate_mismatches']), 1)

    def test_parity_duplicate_missing_and_geometry_fail_closed(self):
        r = self.record([.1, .8, .8, .8])
        with self.assertRaises(ValueError):
            probability_map([r, r])
        with self.assertRaises(ValueError):
            compare(probability_map([r]), {})
        with self.assertRaises(ValueError):
            probability_map([self.record([.1, .8])])

    def test_frozen_thresholds_caps_and_zero_gain(self):
        plan = freeze_plan()
        self.assertEqual(plan['positive'], .7)
        self.assertEqual(plan['minimum_samples_strictly_greater'], 1440)
        self.assertFalse(plan['retuning'])
        for policy in POLICIES:
            values, gains = transform(np.zeros(513, dtype=np.float32), policy)
            self.assertEqual(len(gains), 2)
            self.assertTrue(np.all(values == 0))
            self.assertEqual(gains, [1.0, 1.0])
        values = np.full(1024, .0001, dtype=np.float32)
        self.assertEqual(transform(values, 'peak20')[1][0], 20)
        self.assertEqual(transform(values, 'peak100')[1][0], 100)
        self.assertEqual(transform(values, 'peak300')[1][0], 300)

    def test_slow_gain_block_causal_and_no_clipping(self):
        quiet = np.full(2048, .0001, dtype=np.float32)
        extended = np.concatenate([quiet, np.full(512, .9, dtype=np.float32)])
        a, ga = transform(quiet, 'slow-rms100')
        b, gb = transform(extended, 'slow-rms100')
        np.testing.assert_array_equal(a, b[:2048])
        self.assertEqual(ga, gb[:4])
        self.assertTrue(1 < ga[0] < ga[-1] < 100)
        self.assertLessEqual(float(np.max(np.abs(b))), 1)
        with self.assertRaises(ValueError):
            transform([float('nan')], 'peak20')
        with self.assertRaises(ValueError):
            transform([.1], 'unplanned')

    def test_gain_coverage_pairs_fail_closed(self):
        rows = []
        for suite, count in [('known', 343), ('quantization-pairs', 132), ('new-transforms', 100)]:
            for i in range(count):
                rows.append(dict(id=f'{suite}-{i}', path=f'/{suite}-{i}', suite=suite,
                                 reference='speech', speech=True, samples=2048,
                                 pair_id=str(i // 2), representation='float32' if i % 2 else 'pcm16'))
        validate_manifest(dict(rows=rows))
        rows[343]['representation'] = 'float32'
        with self.assertRaises(ValueError):
            validate_manifest(dict(rows=rows))
        with self.assertRaises(ValueError):
            validate_manifest(dict(rows=rows[:-1]))

    def test_quantization_loss_and_nearest_even(self):
        values = np.array([.000001, .5 / 32768, 1.5 / 32768, -.5 / 32768], dtype=np.float64)
        result = quantize(values)
        np.testing.assert_array_equal(result, [0, 0, 2 / 32768, 0])
        self.assertEqual(result.dtype, np.float32)
        self.assertNotEqual(float(np.float32(values[0])), float(result[0]))


if __name__ == '__main__':
    unittest.main()
