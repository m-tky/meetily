import type { SelectedDevices } from '@/components/DeviceSelection';

type Dependencies = {
  read: () => Promise<Record<string, unknown>>;
  write: (preferences: Record<string, unknown>) => Promise<unknown>;
  commit: (devices: SelectedDevices) => void;
};

/** Serialize read/merge/write; never overwrite unrelated recording preferences.
 * Commit the selection used by Start only after persistence succeeds. */
export function createRecordingInputPreferenceWriter({ read, write, commit }: Dependencies) {
  let queue: Promise<void> = Promise.resolve();
  return (devices: SelectedDevices): Promise<void> => {
    const choice = { ...devices };
    const work = queue.catch(() => {}).then(async () => {
      const previous = await read();
      await write({
        ...previous,
        preferred_mic_device: choice.micDevice,
        preferred_system_device: choice.systemDevice,
      });
      commit(choice);
    });
    queue = work;
    return work;
  };
}
