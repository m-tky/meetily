import { WelcomeStep } from './steps/WelcomeStep';
export function OnboardingFlow({ onComplete }: { onComplete: () => void }) {
  return <WelcomeStep onComplete={onComplete} />;
}
