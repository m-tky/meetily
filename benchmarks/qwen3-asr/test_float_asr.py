import unittest
from evaluate_float_asr import plan, validate


class FloatASRTests(unittest.TestCase):
    def manifest(self):
        rows=[]
        pair=0
        for suite,count in [('baseline',33),('quiet',66),('short-extra',12),('sentence-noise',6),('negative',65)]:
            paired=suite in ('quiet','short-extra','sentence-noise')
            for i in range(count):
                rows.append(dict(id=f'{suite}-{i}',path=f'/{suite}-{i}',suite=suite,
                                 reference='' if suite=='negative' else 'speech',
                                 role='negative' if suite=='negative' else 'speech',samples=16000,
                                 pair_id=f'{suite}-{i//2}' if paired else None,
                                 representation=('pcm16' if i%2 else 'float32') if paired else 'original'))
        return dict(rows=rows)

    def test_frozen_model_gain_and_bypass_scope(self):
        p=plan()
        self.assertEqual(p['threads'],6)
        self.assertEqual(p['asr_input_gain'],1)
        self.assertTrue(p['known_speech_diagnostic_bypass'])
        self.assertFalse(p['negative_bypass'])
        self.assertFalse(p['model_replacement'])
        self.assertFalse(p['correction'])

    def test_missing_duplicate_labels_and_pairs_reject(self):
        manifest=self.manifest();validate(manifest)
        with self.assertRaises(ValueError):validate(dict(rows=manifest['rows'][:-1]))
        rows=self.manifest()['rows'];rows[1]['id']=rows[0]['id']
        with self.assertRaises(ValueError):validate(dict(rows=rows))
        rows=self.manifest()['rows'];rows[-1]['role']='speech'
        with self.assertRaises(ValueError):validate(dict(rows=rows))
        rows=self.manifest()['rows'];rows[33]['representation']='pcm16'
        with self.assertRaises(ValueError):validate(dict(rows=rows))


if __name__=='__main__':unittest.main()
