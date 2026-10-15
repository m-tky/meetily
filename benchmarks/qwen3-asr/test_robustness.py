import math
import unittest

from evaluate_robustness import mix, noise, rms


class Tests(unittest.TestCase):
    def test_seeded_unit_rms(self):
        for kind in ('white', 'lowpass', 'bursts', 'tones'):
            values = noise(kind, 16000)
            self.assertEqual(values, noise(kind, 16000))
            self.assertAlmostEqual(rms(values), 1)

    def test_snr_and_common_gain(self):
        speech = [.7 * math.sin(i) for i in range(16000)]
        background = noise('white', len(speech))
        for db in (20, 10, 0):
            combined, gain = mix(speech, background, db)
            recovered = [m / gain - s for m, s in zip(combined, speech)]
            self.assertAlmostEqual(20 * math.log10(rms(speech) / rms(recovered)), db)
            self.assertLessEqual(max(abs(x) for x in combined), .98000001)


if __name__ == '__main__':
    unittest.main()
