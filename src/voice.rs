use serenity::{
    builder::EditMessage,
    client::Context,
    framework::standard::{Args, CommandResult, macros::command}, model::prelude::Message,
};
use songbird::input::{Compose, YoutubeDl};

use crate::{check_msg, get_http_client};

#[command]
#[only_in(guilds)]
async fn repeat(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild(&ctx.cache).unwrap().id;
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
    let guild_id = msg.guild(&ctx.cache).unwrap().id;

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
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut query = args.rest().to_string();

    let (guild_id, channel_id) = {
        let guild = msg.guild(&ctx.cache).unwrap();
        let channel_id = guild
            .voice_states
            .get(&msg.author.id)
            .and_then(|voice_state| voice_state.channel_id);

        (guild.id, channel_id)
    };

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);
            return Ok(());
        },
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Ok(handler_lock) = manager.join(guild_id, connect_to).await {
        let mut handler = handler_lock.lock().await;
        
        if !query.starts_with("http") {
            query = format!("ytsearch:{}", query);
        } else {
            let _ = msg.clone().edit(&ctx, EditMessage::new().suppress_embeds(true)).await;
        }

        if handler.queue().is_empty() {
            check_msg(msg.channel_id.say(&ctx.http, "sec").await);
        }

        let http_client = get_http_client(ctx).await;

        let mut source = YoutubeDl::new(http_client, query);
        let url = match source.aux_metadata().await {
            Ok(metadata) => {
                metadata.source_url.unwrap_or(String::from(""))
            },
            Err(err) => {
                println!("{:?}", err);
                String::from("")
            },
        };
        let _song = handler.enqueue_input(source.into()).await;

        check_msg(
            msg.channel_id
                .say(&ctx.http, format!("queued: #{} {}", handler.queue().len(), url))
                .await,
        );
    } else {
        check_msg(msg.reply(ctx, "not in vc").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild_id = msg.guild(&ctx.cache).unwrap().id;

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
                .say(&ctx.http, format!("skipping"))
                .await,
        );
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "not in vc").await);
    }

    Ok(())
}
