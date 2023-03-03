use serenity::{
    client::Context,
    framework::{
        standard::{
            macros::command,
            Args,
            CommandResult,
        }
    },
    model::prelude::Message,
    Result as SerenityResult, async_trait,
};

use songbird::{
    EventHandler as VoiceEventHandler, 
    EventContext, 
    Event, input::Restartable, TrackEvent,
};

use std::sync::{atomic::{Ordering, AtomicBool}, Arc};

use crate::RestartTrack;

#[command]
#[only_in(guilds)]
async fn restart(ctx: &Context, msg: &Message) -> CommandResult {
    let val = {
        let restart = {
            let data_read = ctx.data.read().await;
            data_read.get::<RestartTrack>().expect("Expected RestartTrack in TypeMap.").clone()
        };
        let b = restart.load(Ordering::SeqCst);
        let flipped = !b;
        restart.store(flipped, Ordering::SeqCst);
        flipped
    };
    check_msg(msg.channel_id.say(&ctx.http, format!("restart: {}", val)).await);

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

struct TrackEndNotifier {
    restart: Arc<AtomicBool>
}

#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        println!("event");
        if let EventContext::Track(track_list) = ctx {
            println!("tracklist");
            if track_list.len() > 0 {
                println!(">0");
                let first = track_list[0].1;
                if self.restart.load(std::sync::atomic::Ordering::SeqCst) {
                    println!("restart=true");
                    let _result = first.enable_loop();
                } else {
                    println!("restart=false");
                    let _result = first.disable_loop();
                }
            }
        }
        None
    }
}

#[command]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut query = args.rest().to_string();

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "not in vc").await);
            return Ok(());
        },
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (handle_lock, _) = manager.join(guild_id, connect_to).await;

    if !query.starts_with("http") {
        query = format!("ytsearch:{}", query);
    }

    let mut handler = handle_lock.lock().await;

    // Here, we use lazy restartable sources to make sure that we don't pay
    // for decoding, playback on tracks which aren't actually live yet.
    let source = match Restartable::ytdl(query, true).await {
        Ok(source) => source,
        Err(why) => {
            println!("Err starting source: {:?}", why);

            check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

            return Ok(());
        },
    };

    let restart = {
        let data_read = ctx.data.read().await;
        data_read.get::<RestartTrack>().expect("Expected RestartTrack in TypeMap.").clone()
    };
    let restart = Arc::clone(&restart);

    let track_handle = handler.enqueue_source(source.into());
    let _res = track_handle.add_event(Event::Track(TrackEvent::End), TrackEndNotifier { restart });

    check_msg(
        msg.channel_id
            .say(
                &ctx.http,
                format!("queued: #{}", handler.queue().len()),
            )
            .await,
    );

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.skip();

        check_msg(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("skipped: {} in queue.", queue.len()),
                )
                .await,
        );
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "not in vc")
                .await,
        );
    }

    Ok(())
}

/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}