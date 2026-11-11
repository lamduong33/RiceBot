use serenity::{
    all::{Message, Ready, VoiceState},
    async_trait,
    client::{Context, EventHandler},
};
use tracing::info;

use crate::{chat::Chat, greeter::Greeter};

/// Routes Discord events to the bot's features. Serenity runs each event in
/// its own task, so a slow LLM reply never holds up greetings.
pub struct Handler {
    greeter: Greeter,
    chat: Chat,
}

impl Handler {
    pub fn new(greeter: Greeter, chat: Chat) -> Self {
        Self { greeter, chat }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!(user = %ready.user.name, guilds = ready.guilds.len(), "connected to Discord");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        self.chat.on_message(&ctx, &msg).await;
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        self.greeter.on_voice_state_update(&ctx, old.as_ref(), &new).await;
    }
}
