use poise::Context::Prefix;
use serenity::builder::EditMessage;
use songbird::input::{Compose, YoutubeDl};

use crate::{Context, Error};

#[poise::command(prefix_command)]
pub async fn repeat(
    ctx: Context<'_>
) -> Result<(), Error> {
    let Prefix(prefix_ctx) = ctx else {
        return Ok(());
    };
    let serenity_ctx = prefix_ctx.serenity_context;
    let msg = prefix_ctx.msg;

    let guild_id = msg.guild(ctx.cache()).unwrap().id;
    let manager = songbird::get(serenity_ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    if let Some(handler) = manager.get(guild_id) {
        let handler = handler.lock().await;
        if let Some(current) = handler.queue().current() {
            let _ = current.enable_loop();
            ctx.say("Repeating current song").await?;
        } else {
            ctx.say("No songs queued").await?;
        }
    }

    Ok(())
}

#[poise::command(prefix_command)]
pub async fn leave(
    ctx: Context<'_>
) -> Result<(), Error> {
    let Prefix(prefix_ctx) = ctx else {
        return Ok(());
    };
    let serenity_ctx = prefix_ctx.serenity_context;
    let msg = prefix_ctx.msg;

    let guild_id = msg.guild(ctx.cache()).unwrap().id;

    let manager = songbird::get(serenity_ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            ctx.say(format!("Failed: {:?}", e)).await?;
        }
    } else {
        ctx.say("Not in a voice channel").await?;
    }

    Ok(())
}

#[poise::command(prefix_command)]
pub async fn play(
    ctx: Context<'_>,
    query: Vec<String>
) -> Result<(), Error> {
    let Prefix(prefix_ctx) = ctx else {
        return Ok(());
    };
    let serenity_ctx = prefix_ctx.serenity_context;
    let msg = prefix_ctx.msg;

    let mut query = query.join(" ");

    let (guild_id, channel_id) = {
        let guild = msg.guild(ctx.cache()).unwrap();
        let channel_id = guild
            .voice_states
            .get(&msg.author.id)
            .and_then(|voice_state| voice_state.channel_id);

        (guild.id, channel_id)
    };

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            ctx.say("Not in a voice channel").await?;
            return Ok(());
        }
    };

    let manager = songbird::get(serenity_ctx)
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
            ctx.say("sec").await?;
        }

        let http_client = ctx.data().http_client.read().await.clone();

        let mut source = YoutubeDl::new(http_client, query);
        let url = match source.aux_metadata().await {
            Ok(metadata) => {
                metadata.source_url.unwrap_or(String::from(""))
            }
            Err(err) => {
                println!("{:?}", err);
                String::from("")
            }
        };
        let _song = handler.enqueue_input(source.into()).await;

        ctx.say(format!("queued: #{} {}", handler.queue().len(), url)).await?;
    } else {
        ctx.say("not in vc").await?;
    }

    Ok(())
}

#[poise::command(prefix_command)]
pub async fn skip(
    ctx: Context<'_>
) -> Result<(), Error> {
    let Prefix(prefix_ctx) = ctx else {
        return Ok(());
    };
    let serenity_ctx = prefix_ctx.serenity_context;
    let msg = prefix_ctx.msg;

    let guild_id = msg.guild(ctx.cache()).unwrap().id;

    let manager = songbird::get(serenity_ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.skip();

        ctx.say("skipping".to_string()).await?;
    } else {
        ctx.say("not in vc").await?;
    }

    Ok(())
}
