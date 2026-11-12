//! RiceBot: greets people joining voice channels and answers mentions using a
//! locally hosted LLM.

mod chat;
mod config;
mod greeter;
mod handler;
mod llm;
mod people;
mod personality;
mod reply;
mod search;

use anyhow::{Context as _, Result};
use serenity::{Client, all::GatewayIntents};
use songbird::SerenityInit;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{chat::Chat, config::Config, greeter::Greeter, handler::Handler};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env before logging starts so RUST_LOG set there takes effect.
    // A missing .env file is fine; everything can come from the environment.
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rice_bot=info,songbird=warn,serenity=warn")),
        )
        .init();

    let config = Config::from_env()?;
    let greeter = Greeter::load(&config.greeter).await?;
    let chat = Chat::new(&config.chat, &config.llm, &config.search)?;

    // Discord sends message content for messages that mention the bot even
    // without the privileged MESSAGE_CONTENT intent. It's only needed to read
    // the message someone is replying to when they mention the bot.
    let mut intents =
        GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES | GatewayIntents::GUILD_MESSAGES;
    if config.message_content_intent {
        intents |= GatewayIntents::MESSAGE_CONTENT;
    }

    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler(Handler::new(greeter, chat))
        .register_songbird()
        .await
        .context("failed to create Discord client")?;

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        info!("shutting down");
        shard_manager.shutdown_all().await;
    });

    client.start().await.context("Discord client stopped with an error")
}

/// Resolves on Ctrl-C, or on SIGTERM (e.g. `systemctl stop`).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("could not listen for Ctrl-C: {e}");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(e) => {
                tracing::error!("could not listen for SIGTERM: {e}");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
