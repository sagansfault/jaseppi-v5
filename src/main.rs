use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

//use ggstdl::Character;
use serenity::async_trait;
use serenity::model::prelude::Message;
use serenity::prelude::*;
use serenity::framework::standard::macros::{group, command};
use serenity::framework::standard::{StandardFramework, CommandResult, Args};
use serenity::Result as SerenityResult;

use songbird::SerenityInit;

mod voice;
use crate::voice::*;

#[group]
#[commands(leave, play, skip, repeat, /*frames*/)]
struct General;
struct Handler;

#[async_trait]
impl EventHandler for Handler {}

struct RepeatTrack;
impl TypeMapKey for RepeatTrack {
    type Value = Arc<AtomicBool>;
}

// struct GGSTDLCharacterData;
// impl TypeMapKey for GGSTDLCharacterData {
//     type Value = Arc<RwLock<Vec<Character>>>;
// }

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
        data.insert::<RepeatTrack>(Arc::new(AtomicBool::new(false)));

        // let chars = ggstdl::load().await.expect("Could not load ggstdl character data");
        // data.insert::<GGSTDLCharacterData>(Arc::new(RwLock::new(chars)));
    }

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

// #[command]
// #[only_in(guilds)]
// async fn frames(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {

//     if args.len() < 2 {
//         check_msg(msg.channel_id.say(&ctx.http, ".frames <character> <move query>").await);
//         return Ok(());
//     }

//     let Ok(char_query) = args.single::<String>() else {
//         check_msg(msg.channel_id.say(&ctx.http, ".frames <character> <move query>").await);
//         return Ok(());
//     };
//     let Some(move_query) = args.remains() else {
//         check_msg(msg.channel_id.say(&ctx.http, ".frames <character> <move query>").await);
//         return Ok(());
//     };

//     // want to drop the locks and refs asap so other threads can use it
//     let move_found = {
//         let data_read = ctx.data.read().await;
//         let char_data_lock = data_read.get::<GGSTDLCharacterData>().expect("No ggstdl character data in TypeMap").clone();
//         let char_data = char_data_lock.read().await;

//         let character = char_data.iter().find(|c| c.regex.is_match(char_query.as_str()));

//         let Some(character) = character else {
//             check_msg(msg.channel_id.say(&ctx.http, "could not find character").await);
//             return Ok(());
//         };

//         let move_found = character.moves.iter().find(|m| m.regex.is_match(move_query));
//         let Some(move_found) = move_found else {
//             check_msg(msg.channel_id.say(&ctx.http, "could not find move").await);
//             return Ok(());
//         };

//         move_found.clone()
//     };

//     let v = msg.channel_id.send_message(&ctx.http, |m| {
//         m.embed(|e| {
//             e.title(move_found.name)
//                 .field("Damage", move_found.damage, true)
//                 .field("Guard", move_found.guard, true)
//                 .field("Startup", move_found.startup, true)
//                 .field("Active", move_found.active, true)
//                 .field("Recovery", move_found.recovery, true)
//                 .field("On Block", move_found.onblock, true)
//                 .field("Invuln", move_found.invuln, true)
//         })
//     }).await;
//     check_msg(v);

//     Ok(())
// }

// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}