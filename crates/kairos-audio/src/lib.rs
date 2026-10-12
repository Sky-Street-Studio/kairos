//! Named audio channels on top of kira: one sound per channel, with loop, queue and fades.

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::time::Duration;

use kira::sound::PlaybackState;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};

/// The volume group a channel belongs to. `music` and `voice` are their own groups; every other
/// channel name counts as sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bus {
    Music,
    Sound,
    Voice,
}

impl Bus {
    pub fn of(channel: &str) -> Bus {
        match channel {
            "music" => Bus::Music,
            "voice" => Bus::Voice,
            _ => Bus::Sound,
        }
    }
}

/// Decoded audio, ready to play.
#[derive(Clone)]
pub struct Sound(StaticSoundData);

impl Sound {
    pub fn decode(bytes: Vec<u8>) -> Result<Sound, String> {
        StaticSoundData::from_cursor(Cursor::new(bytes)).map(Sound).map_err(|e| e.to_string())
    }
}

/// Converts a 0..=1 slider value to kira's decibels.
pub fn volume_to_decibels(volume: f32) -> Decibels {
    if volume <= 0.001 { Decibels::SILENCE } else { Decibels(20.0 * volume.min(1.0).log10()) }
}

struct Playing {
    asset: String,
    handle: StaticSoundHandle,
    queue: VecDeque<(String, Sound, bool)>,
}

pub struct Audio {
    backend: Option<Backend>,
    channels: HashMap<String, Playing>,
}

struct Backend {
    /// Owns the output stream; dropping it silences every track.
    _manager: AudioManager,
    buses: HashMap<Bus, TrackHandle>,
}

const CUT: Duration = Duration::from_millis(10);

impl Audio {
    /// Opens the default output device. Without one, every call is a silent no-op.
    pub fn new() -> Audio {
        let backend = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .map_err(|e| eprintln!("audio disabled: {e}"))
            .ok()
            .and_then(|mut manager| {
                let mut buses = HashMap::new();
                for bus in [Bus::Music, Bus::Sound, Bus::Voice] {
                    buses.insert(bus, manager.add_sub_track(TrackBuilder::new()).ok()?);
                }
                Some(Backend { _manager: manager, buses })
            });
        Audio { backend, channels: HashMap::new() }
    }

    pub fn silent() -> Audio {
        Audio { backend: None, channels: HashMap::new() }
    }

    pub fn is_enabled(&self) -> bool {
        self.backend.is_some()
    }

    /// Replaces whatever the channel is playing.
    pub fn play(&mut self, channel: &str, asset: &str, sound: Sound, looped: bool, fadein: Option<f64>) {
        if let Some(mut old) = self.channels.remove(channel) {
            old.handle.stop(tween(Some(0.0)));
        }
        let Some(handle) = self.start(channel, sound, looped, fadein) else { return };
        self.channels.insert(channel.to_string(), Playing { asset: asset.to_string(), handle, queue: VecDeque::new() });
    }

    /// Plays after the current sound on the channel finishes, or now if the channel is idle.
    pub fn queue(&mut self, channel: &str, asset: &str, sound: Sound, looped: bool) {
        match self.channels.get_mut(channel) {
            Some(p) if p.handle.state() != PlaybackState::Stopped => {
                p.queue.push_back((asset.to_string(), sound, looped));
            }
            _ => self.play(channel, asset, sound, looped, None),
        }
    }

    pub fn stop(&mut self, channel: &str, fadeout: Option<f64>) {
        if let Some(mut p) = self.channels.remove(channel) {
            p.handle.stop(tween(fadeout));
        }
    }

    /// The asset a channel is playing, if it has not finished.
    pub fn playing(&self, channel: &str) -> Option<&str> {
        let p = self.channels.get(channel)?;
        (p.handle.state() != PlaybackState::Stopped).then_some(p.asset.as_str())
    }

    pub fn channels(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }

    pub fn set_volume(&mut self, bus: Bus, volume: f32) {
        if let Some(track) = self.backend.as_mut().and_then(|b| b.buses.get_mut(&bus)) {
            track.set_volume(volume_to_decibels(volume), tween(Some(0.0)));
        }
    }

    /// Starts queued sounds on channels whose current sound has finished. Call once per frame.
    pub fn update(&mut self) {
        let finished: Vec<String> = self
            .channels
            .iter()
            .filter(|(_, p)| p.handle.state() == PlaybackState::Stopped && !p.queue.is_empty())
            .map(|(c, _)| c.clone())
            .collect();
        for channel in finished {
            let mut p = self.channels.remove(&channel).expect("collected above");
            let (asset, sound, looped) = p.queue.pop_front().expect("filtered non-empty");
            if let Some(handle) = self.start(&channel, sound, looped, None) {
                self.channels.insert(channel, Playing { asset, handle, queue: p.queue });
            }
        }
    }

    fn start(&mut self, channel: &str, sound: Sound, looped: bool, fadein: Option<f64>) -> Option<StaticSoundHandle> {
        let backend = self.backend.as_mut()?;
        let mut data = sound.0;
        if looped {
            data = data.loop_region(..);
        }
        if fadein.is_some() {
            data = data.fade_in_tween(tween(fadein));
        }
        let track = backend.buses.get_mut(&Bus::of(channel))?;
        track.play(data).map_err(|e| eprintln!("audio: cannot play on `{channel}`: {e}")).ok()
    }
}

impl Default for Audio {
    fn default() -> Self {
        Audio::new()
    }
}

fn tween(seconds: Option<f64>) -> Tween {
    let duration = seconds.map(|s| Duration::from_secs_f64(s.max(0.0))).unwrap_or(CUT);
    Tween { duration, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_names_map_to_volume_buses() {
        assert_eq!(Bus::of("music"), Bus::Music);
        assert_eq!(Bus::of("voice"), Bus::Voice);
        assert_eq!(Bus::of("sound"), Bus::Sound);
        assert_eq!(Bus::of("ambience"), Bus::Sound);
    }

    #[test]
    fn slider_volume_maps_to_decibels() {
        assert_eq!(volume_to_decibels(1.0), Decibels::IDENTITY);
        assert_eq!(volume_to_decibels(0.0), Decibels::SILENCE);
        assert!((volume_to_decibels(0.5).0 + 6.02).abs() < 0.01);
    }

    #[test]
    fn a_silent_mixer_accepts_every_call() {
        let mut audio = Audio::silent();
        assert!(!audio.is_enabled());
        audio.stop("music", Some(1.0));
        audio.update();
        assert_eq!(audio.playing("music"), None);
    }
}
