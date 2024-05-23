use poise::{CreateReply, send_reply};
use regex::Regex;
use serenity::all::{AutocompleteChoice, CreateEmbed, CreateEmbedFooter};

use crate::{Context, Error, LazyLock};

#[poise::command(slash_command, prefix_command)]
pub async fn sf6reload(
    ctx: Context<'_>,
) -> Result<(), Error> {
    ctx.say("Reloading SF6 Data...").await?;
    let new_data = sf6rs::framedata::load_all().await;
    let mut guard = ctx.data().sf6.write().await;
    *guard = new_data;
    ctx.say("Reloaded").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn sf6(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_sf6_character"]
    character_query: String,
    #[autocomplete = "autocomplete_sf6_move"]
    move_query: String,
) -> Result<(), Error> {
    let guard = ctx.data().sf6.read().await;
    let result = guard.find_move(&character_query, &move_query);
    if let Err(e) = result {
        ctx.say(format!("{:?}", e)).await?;
        return Ok(());
    }
    let move_found = result.unwrap().clone();
    let mut builder = CreateReply::default();
    let embed = {
        let title = move_found.identifier;
        CreateEmbed::new()
            .title(title)
            .url(move_found.image_link.clone())
            .field("Damage", move_found.damage, true)
            .field("Guard", move_found.guard, true)
            .field("Cancel", move_found.cancel, true)
            .field("Startup", move_found.startup, true)
            .field("Active", move_found.active, true)
            .field("Recovery", move_found.recovery, true)
            .field("On Block", move_found.block_advantage, true)
            .field("On Hit", move_found.hit_advantage, true)
            .field("Armour", move_found.armor, true)
            .field("Invuln", move_found.invuln, true)
            .image(move_found.image_link)
            .footer(CreateEmbedFooter::new(move_found.notes))
    };
    builder = builder.embed(embed);
    send_reply(ctx, builder).await?;
    Ok(())
}

async fn autocomplete_sf6_character(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<String> {
    let partial = &partial.to_lowercase();
    ctx.data().sf6.read().await.character_frame_data.iter()
        .map(|fd| fd.character_id.id.to_string())
        .filter(|s| s.to_lowercase().contains(partial))
        .collect::<Vec<String>>()
}

static CHARACTER_ID_MATCHER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"character_query:(\w+)").unwrap());
async fn autocomplete_sf6_move(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<AutocompleteChoice> {
    let partial = &partial.to_lowercase();

    let invocation_string = ctx.invocation_string();
    let option = CHARACTER_ID_MATCHER.captures(&invocation_string).and_then(|c| c.get(1)).map(|c| c.as_str());
    let Some(character_query) = option else {
        return vec![];
    };
    let character_query = &character_query.to_lowercase();
    let frame_datas = &ctx.data().sf6.read().await.character_frame_data;
    let character_frame_data_opt = frame_datas.iter()
        .find(|c| c.character_id.id.to_lowercase().eq(character_query));
    let Some(character_frame_data) = character_frame_data_opt else {
        return  vec![];
    };
    character_frame_data.moves.iter()
        .filter(|m| m.identifier.to_lowercase().contains(partial) || m.name.to_lowercase().contains(partial))
        .map(|m| AutocompleteChoice::new(format!("{} ({})", m.name, m.identifier), m.identifier.to_string()))
        .collect::<Vec<AutocompleteChoice>>()
}