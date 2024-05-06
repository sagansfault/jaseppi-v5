use std::env;

use ggstdl::GGSTDLData;
use poise::{PrefixFrameworkOptions, serenity_prelude};
use reqwest::Client as HttpClient;
use serenity::all::VoiceState;
use serenity::async_trait;
use serenity::prelude::*;
use sf6rs::framedata::FrameData;
use songbird::SerenityInit;

mod voice;
mod sf6;
mod ggst;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn voice_state_update(&self, ctx: serenity_prelude::Context, old: Option<VoiceState>, new: VoiceState) {
        if let Some(id) = old.and_then(|d| d.channel_id) {
            if let Ok(channel) = id.to_channel(&ctx.http).await {
                if let Ok(members) = channel.guild().unwrap().members(&ctx.cache) {
                    // just bot remaining
                    if members.len() == 1 {
                        for member in members {
                            // not guaranteed to be this bot but whatever, good enough
                            if member.user.bot {
                                let manager = songbird::get(&ctx)
                                    .await
                                    .expect("Songbird Voice client placed in at initialisation.")
                                    .clone();
                                if let Some(guild_id) = new.guild_id {
                                    let _ = manager.leave(guild_id).await;
                                    if let Some(handler) = manager.get(guild_id) {
                                        let handler = handler.lock().await;
                                        handler.queue().modify_queue(|q| q.clear());
                                        let _ = handler.queue().skip();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

struct Data {
    sf6: RwLock<FrameData>,
    ggst: RwLock<GGSTDLData>,
    http_client: RwLock<HttpClient>
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                say(),
                sf6::sf6(), ggst::ggst(),
                voice::leave(), voice::skip(), voice::play(), voice::repeat()
            ],
            prefix_options: PrefixFrameworkOptions {
                prefix: Some(".".into()),
                ..Default::default()
            },
            ..Default::default()
        }).setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let sf6 = sf6rs::framedata::load_all().await;
                let ggst = ggstdl::load().await.expect("Could not load ggstdl character data");
                Ok(Data {
                    sf6: RwLock::new(sf6),
                    ggst: RwLock::new(ggst),
                    http_client: RwLock::new(HttpClient::new())
                })
            })
        }).build();

    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[poise::command(prefix_command)]
async fn say(
    ctx: Context<'_>,
    message: Vec<String>,
) -> Result<(), Error> {
    if message.is_empty() {
        ctx.say("say what?").await?;
    } else if let Context::Prefix(prefix_ctx) = ctx {
        let _ = prefix_ctx.msg.delete(&ctx.http()).await;
        let _ = ctx.channel_id().say(&ctx.http(), message.join(" ")).await;
    }
    Ok(())
}

pub struct LazyLock<T, F = fn() -> T> {
    data: std::sync::OnceLock<T>,
    f: F,
}

impl<T, F> LazyLock<T, F> {
    pub const fn new(f: F) -> LazyLock<T, F> {
        Self {
            data: std::sync::OnceLock::new(),
            f,
        }
    }
}

impl<T> std::ops::Deref for LazyLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data.get_or_init(self.f)
    }
}