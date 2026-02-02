// Environment and paths
use std::fs;

use serenity::all::UserId;
// Serenity - Discord bot functionalities
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

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

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
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
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let mut client = Client::builder(&trimmed_token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    // Shards will automatically attempt to reconnect, and will perform exponential backoff until
    // it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
