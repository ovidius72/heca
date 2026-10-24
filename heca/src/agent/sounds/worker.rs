//! A bounded work queue and the audio device, owned exclusively by a background thread.

use std::{io, sync::{Arc, atomic::{AtomicU64, Ordering}, mpsc::{self, SyncSender}}};

use super::Sound;

pub(super) fn start(capacity: usize, generation: Arc<AtomicU64>) -> io::Result<SyncSender<Sound>> {
    let (sender, receiver) = mpsc::sync_channel::<Sound>(capacity.max(1));
    std::thread::Builder::new().name("heca-agent-sounds".into()).spawn(move || {
        let device = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(device) => device,
            Err(error) => { eprintln!("[heca] sound device unavailable: {error}"); return }
        };
        for sound in receiver {
            if sound.generation != generation.load(Ordering::Relaxed) { continue }
            if let Err(error) = play(&device, &sound) {
                eprintln!("[heca] sound {}: {error}", sound.source);
            }
        }
    })?;
    Ok(sender)
}

fn play(device: &rodio::MixerDeviceSink, sound: &Sound) -> Result<(), String> {
    let source = sound.source.decode()?;
    let player = rodio::Player::connect_new(device.mixer());
    player.set_volume(sound.volume);
    player.append(source);
    player.sleep_until_end();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an audio device and HECA_TEST_SOUND pointing to a short sound file"]
    fn a_configured_file_plays_on_the_real_audio_device() {
        let sound = Sound {
            source: crate::audio::Audio::load(std::env::var_os("HECA_TEST_SOUND").expect("set HECA_TEST_SOUND")),
            volume: 0.1, generation: 0,
        };
        let device = rodio::DeviceSinkBuilder::open_default_sink().unwrap();
        play(&device, &sound).unwrap();
    }
}
