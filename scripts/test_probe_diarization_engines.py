"""No model inference, downloads, or audio devices used by these tests."""
import array
import math
from pathlib import Path
import tempfile
import unittest

from probe_diarization_engines import rasterize, read_wav, require_cpu_simd, sha256, summary, write_float_wav


class EngineProbeTests(unittest.TestCase):
    def test_cpu_simd_guard_rejects_generic_build(self):
        with self.assertRaisesRegex(RuntimeError,'avx2, fma, f16c'):
            require_cpu_simd({'avx2':0,'fma':0,'f16c':0,'llamafile':1})
        with self.assertRaises(RuntimeError):require_cpu_simd({'avx2':1,'fma':1})
        require_cpu_simd({'avx2':1,'fma':1,'f16c':1})

    def test_float_wav_roundtrip_exact(self):
        audio=array.array('f',[0,.1,-.125,1,-1])
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'public-synthetic.wav';write_float_wav(p,audio)
            self.assertEqual(read_wav(p),audio)
            self.assertEqual(len(sha256(p)),64)

    def test_segment_centers_and_clamping(self):
        rows=rasterize([{'speaker':0,'start':-.01,'end':.02}],.04)
        self.assertEqual(rows,[{1},{1},set(),set()])
        rows=rasterize([{'speaker':1,'start':.02,'end':2}],.04)
        self.assertEqual(rows,[set(),set(),{2},{2}])

    def test_overlap_is_not_arbitrarily_resolved(self):
        rows=rasterize([{'speaker':0,'start':0,'end':.02},
                        {'speaker':1,'start':.01,'end':.02}],.02)
        self.assertEqual(rows,[{1},{1,2}])

    def test_reject_invalid_model_output(self):
        for bad in [{'speaker':0,'start':1,'end':0},
                    {'speaker':0,'start':float('nan'),'end':1},
                    {'speaker':-1,'start':0,'end':1},
                    {'speaker':8,'start':0,'end':1}]:
            with self.assertRaises(ValueError):rasterize([bad],1)

    def test_summary_separates_processing_and_pacing(self):
        seg={'speaker':0,'start':0,'end':.02}
        events=[{'push_seconds':.01,'wall_seconds':.12,'input_seconds':.02,'visible_segments':[seg]}]
        stats=summary(events,{'load_seconds':3,'buffer_latency':1.04},
                      {'flush_seconds':.01,'wall_seconds':.13},[seg],[1]*320,
                      array.array('f',[.1]*320),True,.04,10)
        self.assertAlmostEqual(stats['compute_rtf'],1)
        self.assertAlmostEqual(stats['maximum_completion_lateness_seconds'],.1)
        self.assertEqual(stats['clusters'],[0])
        self.assertEqual(stats['proxy_score']['source_identity_accuracy_proxy'],1)
        self.assertEqual(stats['revised_nonempty_frame_events'],0)

    def test_revision_and_unknown_not_claimed_stable(self):
        old={'speaker':0,'start':0,'end':.02}
        new={'speaker':1,'start':0,'end':.02}
        events=[{'push_seconds':.01,'wall_seconds':.02,'input_seconds':.01,'visible_segments':[old]},
                {'push_seconds':.01,'wall_seconds':.04,'input_seconds':.02,'visible_segments':[new]}]
        stats=summary(events,{'load_seconds':0,'buffer_latency':1},
                      {'flush_seconds':0,'wall_seconds':.04},[new],[1]*320,
                      array.array('f',[.1]*320),False,.1,10)
        self.assertEqual(stats['revised_nonempty_frame_events'],2)
        self.assertIsNone(stats['maximum_completion_lateness_seconds'])
        self.assertIsNone(stats['voiced_frame_first_label_age_median_seconds'])


if __name__=='__main__':unittest.main()
