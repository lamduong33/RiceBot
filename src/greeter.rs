//! Greets people joining a voice channel: the bot joins, plays an audio file
//! and leaves once the file has finished playing.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use anyhow::{Context as _, Result};
use serenity::{
    all::{ChannelId, GuildId, VoiceState},
    async_trait,
    client::Context,
};
use songbird::{
    Event, EventContext, EventHandler as VoiceEventHandler, Songbird, TrackEvent,
    error::JoinError,
    input::{
        Input,
        codecs::{get_codec_registry, get_probe},
    },
    tracks::PlayMode,
};
use tokio::{sync::oneshot, time::timeout};
use tracing::{debug, info, warn};

use crate::config::GreeterConfig;

pub struct Greeter {
    /// The encoded audio file, loaded once at startup.
    audio: Arc<[u8]>,
    max_duration: Duration,
    /// Servers where a greeting is in progress. Songbird has a single voice
    /// connection per server, so only one greeting can play in each at a time.
    busy: Mutex<HashSet<GuildId>>,
}

impl Greeter {
    /// Loads the greeting audio and checks that it can be decoded.
    pub async fn load(config: &GreeterConfig) -> Result<Self> {
        let path = &config.audio_path;
        let audio: Arc<[u8]> = std::fs::read(path)
            .with_context(|| format!("could not read greeting audio {}", path.display()))?
            .into();

        Input::from(audio.clone())
            .make_playable_async(get_codec_registry(), get_probe())
            .await
            .with_context(|| format!("could not decode greeting audio {}", path.display()))?;

        Ok(Self {
            audio,
            max_duration: config.max_duration,
            busy: Mutex::default(),
        })
    }

    pub async fn on_voice_state_update(
        &self,
        ctx: &Context,
        old: Option<&VoiceState>,
        new: &VoiceState,
    ) {
        // Only greet people entering voice: ignore moves between channels,
        // mute/deafen changes and leaving.
        let was_in_voice = old.and_then(|state| state.channel_id).is_some();
        let (Some(guild_id), Some(channel_id)) = (new.guild_id, new.channel_id) else {
            return;
        };
        if was_in_voice || is_bot(ctx, new) {
            return;
        }

        let Some(_busy) = BusyGuard::acquire(&self.busy, guild_id) else {
            debug!(%guild_id, "greeting already in progress, skipping");
            return;
        };

        info!(user = %new.user_id, %guild_id, %channel_id, "greeting user");
        if let Err(e) = self.greet(ctx, guild_id, channel_id).await {
            warn!(%guild_id, %channel_id, "greeting failed: {e:#}");
        }
    }

    async fn greet(&self, ctx: &Context, guild_id: GuildId, channel_id: ChannelId) -> Result<()> {
        let manager = songbird::get(ctx)
            .await
            .context("songbird was not registered with the client")?;

        let result = self.play(&manager, guild_id, channel_id).await;

        // Always leave, even if joining or playback failed part way: a failed
        // join can still leave a half-open connection behind.
        match manager.remove(guild_id).await {
            Ok(()) | Err(JoinError::NoCall) => {}
            Err(e) => warn!(%guild_id, "failed to leave voice channel: {e}"),
        }

        result
    }

    async fn play(&self, manager: &Songbird, guild_id: GuildId, channel_id: ChannelId) -> Result<()> {
        let call = manager
            .join(guild_id, channel_id)
            .await
            .context("could not join voice channel")?;

        let track = {
            let mut call = call.lock().await;
            if let Err(e) = call.deafen(true).await {
                debug!(%guild_id, "could not self-deafen: {e}");
            }
            call.play_only_input(Input::from(self.audio.clone()))
        };

        let (done_tx, done_rx) = oneshot::channel();
        let notifier = TrackDone(Arc::new(Mutex::new(Some(done_tx))));
        let registered = track
            .add_event(Event::Track(TrackEvent::End), notifier.clone())
            .and_then(|()| track.add_event(Event::Track(TrackEvent::Error), notifier));
        if registered.is_err() {
            // The track is already gone, so there is nothing left to wait for.
            return Ok(());
        }

        if timeout(self.max_duration, done_rx).await.is_err() {
            warn!(%guild_id, "greeting did not finish within {:?}, stopping it", self.max_duration);
            let _ = track.stop();
        }
        Ok(())
    }
}

fn is_bot(ctx: &Context, state: &VoiceState) -> bool {
    state.user_id == ctx.cache.current_user().id
        || state.member.as_ref().is_some_and(|member| member.user.bot)
}

/// Marks a server as busy for as long as it is held.
struct BusyGuard<'a> {
    busy: &'a Mutex<HashSet<GuildId>>,
    guild_id: GuildId,
}

impl<'a> BusyGuard<'a> {
    fn acquire(busy: &'a Mutex<HashSet<GuildId>>, guild_id: GuildId) -> Option<Self> {
        let inserted = busy.lock().unwrap_or_else(PoisonError::into_inner).insert(guild_id);
        inserted.then_some(Self { busy, guild_id })
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.guild_id);
    }
}

/// Signals once the greeting track has ended or failed.
#[derive(Clone)]
struct TrackDone(Arc<Mutex<Option<oneshot::Sender<()>>>>);

#[async_trait]
impl VoiceEventHandler for TrackDone {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(tracks) = ctx {
            for (state, _) in *tracks {
                if let PlayMode::Errored(e) = &state.playing {
                    warn!("greeting audio failed to play: {e}");
                }
            }
        }

        if let Some(done) = self.0.lock().unwrap_or_else(PoisonError::into_inner).take() {
            let _ = done.send(());
        }
        Some(Event::Cancel)
    }
}
