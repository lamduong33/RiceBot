// Environment and paths
use std::fs;

use serenity::all::{UserId, VoiceState};
// Serenity - Discord bot functionalities
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
// Songbird
// Event related imports to detect track creation failures.
use songbird::events::{Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent};
use songbird::SerenityInit;

// Regex
use regex::Regex;

struct Handler;
// Assuming that llama.cpp and model are in the same folder as Ricebot
const TOKEN_FILE_NAME: &str = "ricebot_token.txt";

/*
 * EventHandler handles events, such as when the bot receives a message or command from the users.
 */
#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event. This is called whenever a new message is received.
    // Event handlers are dispatched through a threadpool, and so multiple events can be
    // dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        let current_user_id: UserId = ctx.cache.current_user().id;
        if msg.mentions_user_id(current_user_id) {
            let message: &str = &msg.content[0..];
            let re: Regex = Regex::new(r"<@\d+>").unwrap();
            let message_without_mention: String = re.replace(&message, "").to_string();
            // Clean the message, remove trailing spaces. Only apply to alphanumeric and whitespaces.
            // TODO: Maybe no need to make sure it's alphanumeric or whitespace.
            let cleaned_message: String = message_without_mention.trim().chars().collect();
            println!("Query: {}", cleaned_message);

            let quoted_message = format!("\"{}\"", cleaned_message);

            println!("Response:{}", quoted_message);

            if let Err(why) = msg
                .channel_id
                .say(&ctx.http, quoted_message.replace("<|im_end|>", ""))
                .await
            {
                println!("Error sending message: {why:?}");
            }
        }
    }

    ///
    /// When a voice channel changes its state (e.g. when a user joins, leaves or moves to a voice channel)
    ///
    /// # Arguments
    ///
    /// * `ctx` - the current context of the event dispatch
    /// * `old` - the old state (if cache is enabled), else will be None
    /// * `new` - the new state of guild's voice channel
    ///
    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        // If old is none, and new has some channel, meaning a user has joined!
        if old
            .as_ref()
            .and_then(|voice_state| voice_state.channel_id)
            .is_none()
            && new.channel_id.is_some()
        {
            if let Some(user_id) = new.member.as_ref().map(|m| m.user.id) {
                println!(
                    "User {} joined voice channel: {:?}",
                    user_id,
                    new.channel_id.unwrap()
                );

                let guild_id = new.guild_id.unwrap();
                let channel_id = new.channel_id.unwrap();

                // Get the song bird voice client from the context
                let manager = songbird::get(&ctx)
                    .await
                    .expect("Songbird placed in at initialisation.")
                    .clone();

                // Join the channel, then perform greeting
                // Join the voice channel
                match manager.join(guild_id, channel_id).await {
                    Ok(handler_lock) => {
                        println!("Joined voice channel with user: {:?}", new.user_id);
                        // Attach an event handler to see notifications of all track errors.
                        let mut handler = handler_lock.lock().await;
                        handler.add_global_event(TrackEvent::Error.into(), TrackErrorNotifier);
                    }
                    Err(e) => {
                        println!("Error joining voice channel: {:?}", e);
                    }
                }
            }
        }
    }

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

struct TrackErrorNotifier;

#[async_trait]
impl VoiceEventHandler for TrackErrorNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (state, handle) in *track_list {
                println!(
                    "Track {:?} encountered an error: {:?}",
                    handle.uuid(),
                    state.playing
                );
            }
        }

        None
    }
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    println!("Looking for token in file {}", TOKEN_FILE_NAME);
    let token: String = fs::read_to_string(TOKEN_FILE_NAME).expect("Error reading file!");
    let trimmed_token: &str = token.trim();
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILDS;

    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let mut client = Client::builder(&trimmed_token, intents)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    // Shards will automatically attempt to reconnect, and will perform exponential backoff until
    // it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
