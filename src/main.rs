use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serenity::async_trait;
use serenity::prelude::*;
use serenity::framework::standard::macros::group;
use serenity::framework::standard::StandardFramework;

use songbird::SerenityInit;

mod voice;
use crate::voice::*;

#[group]
#[commands(leave, play, skip, restart)]
struct General;
struct Handler;

#[async_trait]
impl EventHandler for Handler {}

struct RestartTrack;
impl TypeMapKey for RestartTrack {
    type Value = Arc<AtomicBool>;
}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("."))
        .group(&GENERAL_GROUP);

    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<RestartTrack>(Arc::new(AtomicBool::new(false)));
    }

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}