import Link from 'next/link';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';
import { useConfig } from '@/contexts/ConfigContext';
import { QwenModelSettings } from '@/components/QwenModelSettings';
export function WelcomeStep({onComplete}:{onComplete:()=>void}) {
  const {completeQwenOnboarding,initializationError}=useOnboarding();
  const {setTranscriptModelConfig}=useConfig();
  return <OnboardingContainer title="Welcome to Meetily" description="Local Qwen speech recognition. Cloud text processing is opt-in." hideProgress>
    {initializationError && <p role="alert">{initializationError}</p>}
    <QwenModelSettings automaticOnly onSelect={async model=>{
      setTranscriptModelConfig({provider:'qwen',model,apiKey:null});
      await completeQwenOnboarding();onComplete();
    }}/>
    <Link href="/settings?tab=summaryModels" className="text-sm underline">ChatGPT settings</Link>
  </OnboardingContainer>;
}
