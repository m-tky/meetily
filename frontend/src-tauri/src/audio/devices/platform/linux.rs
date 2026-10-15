use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};

use crate::audio::devices::configuration::{AudioDevice, DeviceType};

/// Configure Linux audio devices using ALSA/PulseAudio
pub fn configure_linux_audio(host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    let mut devices = Vec::new();

    // Add input devices
    match host.input_devices() {
        Ok(inputs) => {
            for device in inputs {
                if let Ok(name) = device.name() {
                    devices.push(AudioDevice::new(name, DeviceType::Input));
                }
            }
        }
        // An unavailable ALSA microphone must not hide usable Pulse monitors.
        Err(error) => log::warn!("ALSA microphone enumeration unavailable: {error}"),
    }

    // Explicit native microphone choices; keep all legacy CPAL names unchanged.
    match crate::audio::capture::pulse::input_names() {
        Ok(names) => devices.extend(names.into_iter().map(|name| AudioDevice::new(name, DeviceType::Input))),
        Err(e) => log::warn!("Pulse microphone discovery unavailable: {e}"),
    }

    // Native Pulse introspection includes PipeWire's Pulse compatibility server.
    match crate::audio::capture::pulse::monitor_names() {
        Ok(names) => devices.extend(names.into_iter().map(|name| AudioDevice::new(name, DeviceType::Output))),
        Err(e) => log::warn!("Pulse monitor discovery unavailable: {e}"),
    }

    // Preserve legacy ALSA monitor inputs as an optional fallback.
    if let Ok(pulse_host) = cpal::host_from_id(cpal::HostId::Alsa) {
        if let Ok(inputs) = pulse_host.input_devices() {
            for device in inputs {
                if let Ok(name) = device.name() {
                    // Check if it's a monitor source
                    if name.contains("monitor") {
                        devices.push(AudioDevice::new(
                            // CPAL lookup uses exact names; labels must not alter device identity.
                            name,
                            DeviceType::Output
                        ));
                    }
                }
            }
        }
    }

    Ok(devices)
}