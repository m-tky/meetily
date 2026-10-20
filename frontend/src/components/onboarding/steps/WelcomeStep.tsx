import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { QwenSetupInfo } from '@/lib/qwen-setup';
import { Lock, Sparkles, Cpu } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';
import Link from 'next/link';
import { QwenModelSettings } from '@/components/QwenModelSettings';
import { useConfig } from '@/contexts/ConfigContext';

export function WelcomeStep({ onComplete }: { onComplete: () => void }) {
  const { goNext, completeQwenOnboarding } = useOnboarding();
  const { setTranscriptModelConfig } = useConfig();
  const [qwenAvailable, setQwenAvailable] = useState(false);
  useEffect(() => {
    let active = true;
    invoke<QwenSetupInfo>('qwen_setup_info').then(info => { if (active) setQwenAvailable(info.available); }).catch(() => {});
    return () => { active = false; };
  }, []);

  const features = [
    {
      icon: Lock,
      title: 'Audio stays local. Cloud text processing is opt-in.',
    },
    {
      icon: Sparkles,
      title: 'Intelligent summaries & insights',
    },
    {
      icon: Cpu,
      title: 'Works offline, no cloud required',
    },
  ];

  return (
    <OnboardingContainer
      title="Welcome to Meetily"
      description="Record locally. Choose local AI or your ChatGPT plan for minutes."
      step={1}
      hideProgress={true}
    >
      <div className="flex flex-col items-center space-y-10">
        {/* Divider */}
        <div className="w-16 h-px bg-gray-300" />

        {/* Features Card */}
        <div className="w-full max-w-md bg-white rounded-lg border border-gray-200 shadow-sm p-6 space-y-4">
          {features.map((feature, index) => {
            const Icon = feature.icon;
            return (
              <div key={index} className="flex items-start gap-3">
                <div className="flex-shrink-0 mt-0.5">
                  <div className="w-5 h-5 rounded-full bg-gray-100 flex items-center justify-center">
                    <Icon className="w-3 h-3 text-gray-700" />
                  </div>
                </div>
                <p className="text-sm text-gray-700 leading-relaxed">{feature.title}</p>
              </div>
            );
          })}
        </div>

        {qwenAvailable && <div className="w-full max-w-md">
          <QwenModelSettings automaticOnly onSelect={async () => {
            setTranscriptModelConfig({ provider: 'qwen', model: 'Qwen3-ASR-1.7B-INT8', apiKey: null });
            await completeQwenOnboarding();
            onComplete();
          }} />
        </div>}

        <Link href="/settings?tab=summaryModels" className="text-sm underline">ChatGPT settings</Link>

        {/* CTA Section */}
        <div className="w-full max-w-xs space-y-3">
          <Button
            onClick={goNext}
            className="w-full h-11 bg-gray-900 hover:bg-gray-800 text-white"
          >
            {qwenAvailable ? 'Set up Parakeet and local summaries' : 'Get Started'}
          </Button>
          {qwenAvailable && <p className="text-xs text-center text-gray-500">Choose Qwen above for transcription-only setup. No local summary model is required.</p>}
        </div>
      </div>
    </OnboardingContainer>
  );
}
