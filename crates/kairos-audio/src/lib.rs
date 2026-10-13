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
pub struct Sound {
    data: StaticSoundData,
    loop_region: Option<(f64, Option<f64>)>,
}

impl Sound {
    pub fn decode(bytes: Vec<u8>) -> Result<Sound, String> {
        StaticSoundData::from_cursor(Cursor::new(bytes))
            .map(|data| Sound { data, loop_region: None })
            .map_err(|e| e.to_string())
    }

    pub fn with_loop_region(mut self, start: f64, end: Option<f64>) -> Sound {
        self.loop_region = Some((start, end));
        self
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
    fading: Vec<(Bus, StaticSoundHandle)>,
    music_volume: f32,
    duck: f32,
    duck_fade: f64,
    ducked: bool,
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
        Audio {
            backend,
            channels: HashMap::new(),
            fading: Vec::new(),
            music_volume: 1.0,
            duck: 1.0,
            duck_fade: 0.25,
            ducked: false,
        }
    }

    pub fn silent() -> Audio {
        Audio {
            backend: None,
            channels: HashMap::new(),
            fading: Vec::new(),
            music_volume: 1.0,
            duck: 1.0,
            duck_fade: 0.25,
            ducked: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.backend.is_some()
    }

    /// Replaces whatever the channel is playing.
    pub fn play(&mut self, channel: &str, asset: &str, sound: Sound, looped: bool, fadein: Option<f64>) {
        if let Some(mut old) = self.channels.remove(channel) {
            old.handle.stop(tween(replacement_fadeout(channel, fadein)));
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
            if Bus::of(channel) == Bus::Voice && fadeout.is_some_and(|seconds| seconds > 0.0) {
                self.fading.push((Bus::Voice, p.handle));
            }
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
        if bus == Bus::Music {
            self.music_volume = volume;
            self.apply_music_volume(0.0);
        } else if let Some(track) = self.backend.as_mut().and_then(|b| b.buses.get_mut(&bus)) {
            track.set_volume(volume_to_decibels(volume), tween(Some(0.0)));
        }
    }

    pub fn set_ducking(&mut self, amount: f32, fade: f64) {
        self.duck = amount.clamp(0.0, 1.0);
        self.duck_fade = fade.max(0.0);
        self.update_ducking();
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
        self.fading.retain(|(_, handle)| handle.state() != PlaybackState::Stopped);
        self.update_ducking();
    }

    fn start(&mut self, channel: &str, sound: Sound, looped: bool, fadein: Option<f64>) -> Option<StaticSoundHandle> {
        let backend = self.backend.as_mut()?;
        let mut data = sound.data;
        if looped {
            data = match sound.loop_region {
                Some((start, Some(end))) => data.loop_region(start..end),
                Some((start, None)) => data.loop_region(start..),
                None => data.loop_region(..),
            };
        }
        if fadein.is_some() {
            data = data.fade_in_tween(tween(fadein));
        }
        let track = backend.buses.get_mut(&Bus::of(channel))?;
        track.play(data).map_err(|e| eprintln!("audio: cannot play on `{channel}`: {e}")).ok()
    }

    fn update_ducking(&mut self) {
        let ducked =
            self.fading.iter().any(|(bus, handle)| *bus == Bus::Voice && handle.state() != PlaybackState::Stopped)
                || self.channels.iter().any(|(channel, playing)| {
                    Bus::of(channel) == Bus::Voice && playing.handle.state() != PlaybackState::Stopped
                });
        if ducked != self.ducked {
            self.ducked = ducked;
            self.apply_music_volume(self.duck_fade);
        }
    }

    fn apply_music_volume(&mut self, fade: f64) {
        let volume = effective_music_volume(self.music_volume, self.duck, self.ducked);
        if let Some(track) = self.backend.as_mut().and_then(|b| b.buses.get_mut(&Bus::Music)) {
            track.set_volume(volume_to_decibels(volume), tween(Some(fade)));
        }
    }
}

impl Default for Audio {
    fn default() -> Self {
        Audio::new()
    }
}

fn replacement_fadeout(channel: &str, fadein: Option<f64>) -> Option<f64> {
    (Bus::of(channel) == Bus::Music).then_some(fadein).flatten()
}

fn effective_music_volume(volume: f32, duck: f32, ducked: bool) -> f32 {
    volume * if ducked { duck } else { 1.0 }
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
    fn music_replacement_uses_the_new_track_fadein_as_its_fadeout() {
        assert_eq!(replacement_fadeout("music", Some(1.5)), Some(1.5));
        assert_eq!(replacement_fadeout("music", None), None);
        assert_eq!(replacement_fadeout("voice", Some(1.5)), None);
    }

    #[test]
    fn voice_ducking_multiplies_music_volume_only_while_voice_is_active() {
        assert_eq!(effective_music_volume(0.8, 0.5, true), 0.4);
        assert_eq!(effective_music_volume(0.8, 0.5, false), 0.8);
    }

    #[test]
    fn loop_points_are_attached_to_the_decoded_sound() {
        let mut wav = vec![0; 44 + 8_000 * 2];
        wav[0..4].copy_from_slice(b"RIFF");
        let file_size = (wav.len() - 8) as u32;
        wav[4..8].copy_from_slice(&file_size.to_le_bytes());
        wav[8..16].copy_from_slice(b"WAVEfmt ");
        wav[16..20].copy_from_slice(&16u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1u16.to_le_bytes());
        wav[22..24].copy_from_slice(&1u16.to_le_bytes());
        wav[24..28].copy_from_slice(&8_000u32.to_le_bytes());
        wav[28..32].copy_from_slice(&16_000u32.to_le_bytes());
        wav[32..34].copy_from_slice(&2u16.to_le_bytes());
        wav[34..36].copy_from_slice(&16u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");
        wav[40..44].copy_from_slice(&(16_000u32).to_le_bytes());
        let sound = Sound::decode(wav).unwrap().with_loop_region(1.25, Some(3.0));
        assert_eq!(sound.loop_region, Some((1.25, Some(3.0))));
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
