"""Pure probe checks: no native model download or audio device access."""
import array
import math
from pathlib import Path
import struct
import tempfile
import unittest

from probe_nvidia_diarization import labels, make_fixture, proxy_score, read_wav


def wav_bytes(samples, kind=1, bits=16, rate=16000):
    payload = struct.pack('<' + ('h' if bits == 16 else 'f')*len(samples), *samples)
    fmt = struct.pack('<HHIIHH', kind, 1, rate, rate*bits//8, bits//8, bits)
    chunks = b'JUNK' + struct.pack('<I', 1) + b'x\0'
    chunks += b'fmt ' + struct.pack('<I', len(fmt)) + fmt
    chunks += b'data' + struct.pack('<I', len(payload)) + payload
    return b'RIFF' + struct.pack('<I', len(chunks)+4) + b'WAVE' + chunks


class ProbeTests(unittest.TestCase):
    def test_pcm16_and_float32_with_odd_chunk(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'public-test.wav'
            path.write_bytes(wav_bytes([0, 16384, -32768]))
            self.assertEqual(list(read_wav(path)), [0, .5, -1])
            path.write_bytes(wav_bytes([0., .25], 3, 32))
            self.assertEqual(list(read_wav(path)), [0., .25])

    def test_reject_rate_and_nonfinite(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'test.wav'
            for raw in (wav_bytes([1], rate=48000), wav_bytes([float('nan')], 3, 32), b'broken'):
                path.write_bytes(raw)
                with self.assertRaises(ValueError):
                    read_wav(path)

    def test_fixture_preserves_exact_sample_count_and_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            sources = [Path(tmp)/f'{i}.wav' for i in range(2)]
            for i, path in enumerate(sources):
                path.write_bytes(wav_bytes([8000*(i+1)]*1600))
            audio, ref, provenance = make_fixture('turns', sources, 5)
            self.assertEqual(len(audio), 80000)
            self.assertEqual(len(ref), len(audio))
            self.assertEqual(set(ref), {0, 1, 2})
            self.assertEqual(len(provenance), 2)
            self.assertTrue(all(x in (0., 8000/32768, 16000/32768) for x in audio))

    def test_labels_are_anonymous_and_thresholded(self):
        self.assertEqual(labels([.1, .9, .2, .3, .7, .6], 2), [2, 0, 1])

    def test_permutation_score_and_missing_speech_penalty(self):
        audio = array.array('f', [.1]*320)
        ref = [1]*160 + [2]*160
        correct = proxy_score(audio, ref, [2, 1], .01, 2)
        self.assertEqual(correct['source_identity_accuracy_proxy'], 1)
        missed = proxy_score(audio, ref, [0, 1], .01, 2)
        self.assertEqual(missed['source_identity_accuracy_proxy'], .5)
        collapsed = proxy_score(audio, ref, [1, 1], .01, 2)
        self.assertEqual(collapsed['source_identity_accuracy_proxy'], .5)

    def test_negative_signal_and_false_speaker_counter(self):
        for kind in ('silence', 'tone'):
            audio, ref, sources = make_fixture(kind, [], .1)
            self.assertEqual(len(audio), 1600)
            self.assertTrue(all(math.isfinite(x) for x in audio))
            self.assertEqual(sources, [])
            score = proxy_score(audio, ref, [0]*9+[1], .01, 2)
            self.assertEqual(score['voiced_frames'], 0)
            self.assertEqual(score['false_speaker_frames'], 1)


if __name__ == '__main__':
    unittest.main()
