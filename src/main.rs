use std::env::current_dir;
// Environment and paths
use std::{fs, path};

// Serenity - Discord bot functionalities
use serenity::async_trait;
use serenity::http::Typing;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

// For running commands
use std::process::Command;

// Regex
use regex::Regex;

struct Handler;
// Assuming that llama.cpp and model are in the same folder as Ricebot
const TOKEN_LOCATION: &str = "ricebot_token.txt";

fn send_prompt_to_server(prompt: String) -> String {
    let mut output = "";
    if prompt != "" {
        let api_url: String = "127.0.0.1:8080".to_string();
    }
    return output.to_string();
}

fn get_current_dir() -> &str {
    let current_path = current_dir().unwrap().to_str();
    return current_path;
}

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event. This is called whenever a new message is received.
    // Event handlers are dispatched through a threadpool, and so multiple events can be
    // dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        let current_user_id = ctx.cache.current_user().id;
        if msg.mentions_user_id(current_user_id) {
            // Sending a message can fail, due to a network error, an authentication error, or lack
            // of permissions to post in the channel, so log to stdout when some error happens,
            // with a description of it.
            let typing = Typing::start(ctx.http.clone(), msg.channel_id);
            let message = &msg.content[0..];
            let re = Regex::new(r"<@\d+>").unwrap();
            let message_without_mention = re.replace(&message, "").to_string();
            let cleaned_message: String = message_without_mention
                .trim()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect();
            println!("Query: {}", cleaned_message);

            // NOTE: Assuming that the executable is ran at the root of the project.
            // e.g. running `sh start.sh` at the project's root directory.
            let current_path = current_dir().unwrap().to_str();
            let ricechat_relative_path = "/src/chat.sh";
            let ricechat_path = current_path.to_owned() + ricechat_relative_path.to_owned();
            let quoted_message = format!("\"{}\"", cleaned_message);

            let output = Command::new("sh")
                .arg(ricechat_path)
                .arg(quoted_message.clone())
                .output()
                .expect("failed to execute command");

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            println!("Output:{}", stdout);

            typing.stop();
            if let Err(why) = msg
                .channel_id
                .say(&ctx.http, stdout.replace("<|im_end|>", ""))
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
    println!("Looking for token in file {}", TOKEN_LOCATION);
    let contents = fs::read_to_string(TOKEN_LOCATION).expect("Error reading file!");
    let contents = contents.trim();
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let mut client = Client::builder(&contents, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform exponential backoff until
    // it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
