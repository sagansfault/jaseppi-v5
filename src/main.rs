use std::env;
use std::sync::Arc;

use ggstdl::{GGSTDLData, GGSTDLError};
use rand::prelude::SliceRandom;
use serenity::async_trait;
use serenity::model::prelude::Message;
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use serenity::framework::standard::macros::{group, command};
use serenity::framework::standard::{StandardFramework, CommandResult, Args};
use serenity::Result as SerenityResult;

use songbird::SerenityInit;

mod voice;
use crate::voice::*;

#[group]
#[commands(leave, play, skip, repeat, fd, say)]
struct General;
struct Handler;

const EIGHT_BALL_ANSWERS: [&str; 10] = [
    "It is certain",    "Don’t count on it",
    "It is decidedly so",	"My reply is no",
    "Without a doubt",	"My sources say no",
    "Yes definitely",	"Outlook not so good",
    "You may rely on it",	"Very doubtful",
];

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if let Ok(true) = msg.mentions_me(&ctx.http).await {
            if msg.content.ends_with("?") {
                let text = EIGHT_BALL_ANSWERS.choose(&mut rand::thread_rng()).unwrap_or(&"idk");
                check_msg(msg.channel_id.say(&ctx.http, text).await);
            }
        }
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        if let Some(id) = old.map(|d| d.channel_id).flatten() {
            if let Ok(channel) = id.to_channel(&ctx.http).await {
                if let Ok(members) = channel.guild().unwrap().members(&ctx.cache).await {
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

struct GGSTDLCharacterData;
impl TypeMapKey for GGSTDLCharacterData {
    type Value = Arc<RwLock<GGSTDLData>>;
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
        let ggstdldata = ggstdl::load().await.expect("Could not load ggstdl character data");
        data.insert::<GGSTDLCharacterData>(Arc::new(RwLock::new(ggstdldata)));
    }

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[command]
#[only_in(guilds)]
async fn say(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if args.is_empty() {
        check_msg(msg.channel_id.say(&ctx.http, ".say <message>").await);
        return Ok(());
    }

    let to_say = args.rest();
    if let Ok(_) = msg.delete(&ctx.http).await {
        let _ = msg.channel_id.say(&ctx.http, to_say.clone()).await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn fd(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {

    if args.len() < 2 {
        check_msg(msg.channel_id.say(&ctx.http, ".fd <character> <move query>").await);
        return Ok(());
    }

    let Ok(char_query) = args.single::<String>() else {
        check_msg(msg.channel_id.say(&ctx.http, ".fd <character> <move query>").await);
        return Ok(());
    };
    let Some(move_query) = args.remains() else {
        check_msg(msg.channel_id.say(&ctx.http, ".fd <character> <move query>").await);
        return Ok(());
    };

    // want to drop the locks and refs asap so other threads can use it
    let move_found = {
        let data_read = ctx.data.read().await;
        let ggstdl_data_lock = data_read.get::<GGSTDLCharacterData>().expect("No ggstdl character data in TypeMap").clone();
        let ggstdl_data = ggstdl_data_lock.read().await;

        let res = ggstdl_data.find_move(char_query.as_str(), move_query);
        let Ok(move_found) = res else {
            let err_msg = match res.unwrap_err() {
                GGSTDLError::UnknownCharacter => "could not find character",
                GGSTDLError::UnknownMove => "could not find move",
            };
            check_msg(msg.channel_id.say(&ctx.http, err_msg).await);
            return Ok(());
        };
        move_found.clone()
    };
        
    let v = msg.channel_id.send_message(&ctx.http, |m| {
        m.embed(|e| {
            let title = {
                if move_found.input.eq_ignore_ascii_case(&move_found.name) {
                    move_found.input
                } else {
                    format!("{} ({})", move_found.name, move_found.input)
                }
            };
            e.title(title)
                .field("Damage", move_found.damage, true)
                .field("Guard", move_found.guard, true)
                .field("Startup", move_found.startup, true)
                .field("Active", move_found.active, true)
                .field("Recovery", move_found.recovery, true)
                .field("On Block", move_found.onblock, true)
                .field("Invuln", move_found.invuln, true)
                .image(move_found.hitboxes)
        })
    }).await;
    check_msg(v);

    Ok(())
}

// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}