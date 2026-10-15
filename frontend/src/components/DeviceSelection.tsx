import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { RefreshCw, Mic, Speaker } from 'lucide-react';
import { AudioBackendSelector } from './AudioBackendSelector';

export interface AudioDevice {
  name: string;
  device_type: 'Input' | 'Output';
}
export interface SelectedDevices {
  micDevice: string | null;
  systemDevice: string | null;
}

interface DeviceSelectionProps {
  selectedDevices: SelectedDevices;
  onDeviceChange: (devices: SelectedDevices) => void;
  disabled?: boolean;
  compact?: boolean;
  onAvailabilityChange?: (available: boolean, selection: SelectedDevices) => void;
}

export function deviceValue(device: AudioDevice) {
  return `${device.name} (${device.device_type.toLowerCase()})`;
}
export function selectedDeviceAvailable(choice: string | null, devices: AudioDevice[]) {
  return choice === null || devices.some(d => choice === d.name || choice === deviceValue(d));
}
function speakerLabel(name: string) {
  return name.startsWith('pulse-monitor:')
    ? name.slice('pulse-monitor:'.length).replace(/\.monitor$/, '')
    : name;
}

/** Enumeration only: no idle capture, fake level meter, or automatic fallback. */
export function DeviceSelection({ selectedDevices, onDeviceChange, disabled = false,
  compact = false, onAvailabilityChange }: DeviceSelectionProps) {
  const id = useId();
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestId = useRef(0);
  const fetchDevices = useCallback(async () => {
    const current = ++requestId.current;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<AudioDevice[]>('get_audio_devices');
      if (current === requestId.current) setDevices(result);
    } catch {
      if (current === requestId.current) setError('Could not load input devices. Refresh to retry.');
    } finally {
      if (current === requestId.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    void fetchDevices();
    return () => { requestId.current += 1; };
  }, [fetchDevices]);

  const inputs = devices.filter(d => d.device_type === 'Input');
  const outputs = devices.filter(d => d.device_type === 'Output');
  const micMissing = !selectedDeviceAvailable(selectedDevices.micDevice, inputs);
  const systemMissing = !selectedDeviceAvailable(selectedDevices.systemDevice, outputs);
  const available = !loading && !error && devices.length > 0 && !micMissing && !systemMissing;
  useEffect(() => { onAvailabilityChange?.(available, selectedDevices); },
    [available, selectedDevices.micDevice, selectedDevices.systemDevice, onAvailabilityChange]);

  function picker(kind: 'Input' | 'Output', choice: string | null, options: AudioDevice[], missing: boolean) {
    const label = kind === 'Input' ? 'Microphone' : 'PC audio';
    const suffix = kind === 'Input' ? 'mic' : 'system';
    const recognized = options.find(d => choice === d.name || choice === deviceValue(d));
    const value = choice === null ? '__default__' : recognized ? deviceValue(recognized) : choice;
    // Value remains a native device identity, not the friendly display label.
    return <div className="space-y-1.5">
      <label htmlFor={`${id}-${suffix}`} className="flex items-center gap-2 text-sm font-medium text-gray-700">
        {kind === 'Input' ? <Mic className="h-4 w-4" /> : <Speaker className="h-4 w-4" />}{label}
      </label>
      <select id={`${id}-${suffix}`} aria-label={label} value={value}
        disabled={disabled || loading || !!error}
        className="w-full min-w-0 rounded-md border border-gray-300 bg-white px-3 py-2 text-sm disabled:opacity-60"
        onChange={event => {
          if (disabled || loading || error) return;
          const next = event.target.value === '__default__' ? null : event.target.value;
          onDeviceChange({ ...selectedDevices, [kind === 'Input' ? 'micDevice' : 'systemDevice']: next });
        }}>
        <option value="__default__">{kind === 'Input' ? 'Default microphone' : 'Default PC audio'}</option>
        {missing && choice && <option value={choice} disabled>{choice} (unavailable)</option>}
        {options.map(device => <option key={deviceValue(device)} value={deviceValue(device)}>
          {kind === 'Input'
            ? device.name.startsWith('pulse-input:')
              ? device.name.slice('pulse-input:'.length)
              : device.name
            : speakerLabel(device.name)}
        </option>)}
      </select>
      {!loading && !error && options.length === 0 && <p className="text-xs text-gray-500">
        {kind === 'Input' ? 'No microphone available.' : 'No PC audio available.'}
      </p>}
    </div>;
  }

  return <div className="space-y-3">
    <div className="flex items-center justify-between">
      <h4 className="text-sm font-semibold text-gray-900">Audio inputs</h4>
      <button type="button" aria-label="Refresh audio inputs" title="Refresh audio inputs"
        disabled={disabled || loading} onClick={() => void fetchDevices()}
        className="rounded p-1.5 hover:bg-gray-100 disabled:opacity-50">
        <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
      </button>
    </div>
    {loading && <p role="status" className="text-xs text-gray-500">Loading inputs…</p>}
    {error && <p role="alert" className="text-sm text-red-700">{error}</p>}
    {!loading && !error && (micMissing || systemMissing) && <p role="alert" className="text-sm text-amber-800">
      Selected device unavailable. Choose another device.
    </p>}
    <div className={compact ? 'grid grid-cols-1 gap-3 sm:grid-cols-2' : 'space-y-3'}>
      {picker('Input', selectedDevices.micDevice, inputs, micMissing)}
      {picker('Output', selectedDevices.systemDevice, outputs, systemMissing)}
    </div>
    {!compact && !disabled && <AudioBackendSelector disabled={disabled} />}
  </div>;
}
