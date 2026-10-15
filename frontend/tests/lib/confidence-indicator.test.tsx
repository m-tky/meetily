import React from 'react';
import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { ConfidenceIndicator } from '../../src/components/ConfidenceIndicator';

test('unknown/nonfinite confidence does not display a fabricated percentage', () => {
  for (const value of [-1, NaN, Infinity, 1.1]) {
    expect(renderToStaticMarkup(<ConfidenceIndicator confidence={value} />)).toBe('');
  }
  expect(renderToStaticMarkup(<ConfidenceIndicator confidence={0.7} />)).toContain('70%');
  expect(renderToStaticMarkup(<ConfidenceIndicator confidence={0} />)).toContain('0%');
});
