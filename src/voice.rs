use serenity::{
    client::Context,
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
};

use songbird::input::Restartable;

use crate::check_msg;

#[command]
#[only_in(guilds)]
async fn repeat(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    if let Some(handler) = manager.get(guild_id) {
        let handler = handler.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.enable_loop();
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Repeating current song")
                    .await,
            );
        } else {
            check_msg(msg.channel_id.say(&ctx.http, "No songs queued (yet)").await);
        }
    }

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
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "not in vc").await);
    }

    Ok(())
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
        }
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
    if handler.queue().is_empty() {
        check_msg(msg.channel_id.say(&ctx.http, "sec").await);
    }

    // Here, we use lazy restartable sources to make sure that we don't pay
    // for decoding, playback on tracks which aren't actually live yet.
    let source = match Restartable::ytdl(query, true).await {
        Ok(source) => source,
        Err(why) => {
            println!("Err starting source: {:?}", why);

            check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

            return Ok(());
        }
    };

    let _track_handle = handler.enqueue_source(source.into());

    check_msg(
        msg.channel_id
            .say(&ctx.http, format!("queued: #{}", handler.queue().len()))
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
                .say(&ctx.http, format!("skipped: {} in queue.", queue.len()))
                .await,
        );
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "not in vc").await);
    }

    Ok(())
}
