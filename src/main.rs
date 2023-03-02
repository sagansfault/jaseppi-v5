use std::env;

use serenity::async_trait;
use serenity::prelude::*;
use serenity::framework::standard::macros::group;
use serenity::framework::standard::StandardFramework;

use songbird::SerenityInit;

mod voice;
use crate::voice::*;

#[group]
#[commands(leave, play, skip)]
struct General;
struct Handler;

#[async_trait]
impl EventHandler for Handler {}

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

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}