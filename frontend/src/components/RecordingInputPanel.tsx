'use client';

import { DeviceSelection, type SelectedDevices } from '@/components/DeviceSelection';
import { useConfig } from '@/contexts/ConfigContext';

export function RecordingInputPanel({ disabled, onAvailabilityChange }: {
  disabled: boolean;
  onAvailabilityChange: (available: boolean, selection: SelectedDevices) => void;
}) {
  const { selectedDevices, updateSelectedDevices, devicePreferencesLoaded,
    isSavingDevices, devicePreferencesError } = useConfig();
  return <section aria-label="Recording inputs" className="mx-4 mb-3 rounded-lg border bg-white p-3 shadow-sm">
    <DeviceSelection compact selectedDevices={selectedDevices}
      onDeviceChange={devices => { void updateSelectedDevices(devices).catch(() => {}); }}
      disabled={disabled || !devicePreferencesLoaded || isSavingDevices}
      onAvailabilityChange={onAvailabilityChange} />
    {(!devicePreferencesLoaded || isSavingDevices) && <p role="status" aria-live="polite" className="mt-2 text-xs text-gray-500">
      {isSavingDevices ? 'Saving…' : 'Loading…'}
    </p>}
    {devicePreferencesError && <p role="alert" className="mt-2 text-sm text-red-700">{devicePreferencesError}</p>}
  </section>;
}
