import random
import unittest

from diagnose_gate import endpoints
from test_vad_reference import reference


class Tests(unittest.TestCase):
    def test_default_scalar_parity(self):
        rng = random.Random(512)
        # Values away from boundary literals; separate equality test below.
        for _ in range(300):
            n = rng.randint(1, 200)
            p = [rng.choice([0, .1, .4, .6, .9]) for _ in range(n)]
            length = 512 * n - rng.randrange(512)
            self.assertEqual(endpoints(p, length), reference(p, length))

    def test_threshold_and_real_eof(self):
        self.assertEqual(endpoints([.9], 1), [])
        self.assertEqual(endpoints([.9] * 4, 1537)[0]['end_sample'], 1537)
        self.assertEqual(endpoints([.9] * 4, 1537, minimum=3200), [])
        # Equality at negative threshold must NOT start candidate silence.
        self.assertEqual(endpoints([.9] + [.35] * 40, 41 * 512)[0]['end_sample'], 41 * 512)
        self.assertEqual(endpoints([.9] + [.349] * 40, 41 * 512), [])

    def test_invalid_probabilities(self):
        for p in ([float('nan')], [1.1], []):
            with self.assertRaises(ValueError):
                endpoints(p, 512)


if __name__ == '__main__':
    unittest.main()
