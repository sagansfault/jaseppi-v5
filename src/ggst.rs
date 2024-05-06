use poise::{CreateReply, send_reply};
use regex::Regex;
use serenity::all::{AutocompleteChoice, CreateEmbed};

use crate::{Context, Error, LazyLock};

#[poise::command(slash_command)]
pub async fn ggst(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_ggst_character"]
    character_query: String,
    #[autocomplete = "autocomplete_ggst_move"]
    move_query: String,
) -> Result<(), Error> {
    let guard = ctx.data().ggst.read().await;
    let result = guard.find_move(&character_query, &move_query);
    if let Err(e) = result {
        ctx.say(format!("{:?}", e)).await?;
        return Ok(());
    }
    let move_found = result.unwrap().clone();

    let url = move_found.hitboxes.first().map(|s| s.as_str()).unwrap_or("https://www.dustloop.com/w/GGST");
    let mut builder = CreateReply::default();
    let mut embed = {
        let title = {
            if move_found.input.eq_ignore_ascii_case(&move_found.name) {
                move_found.input
            } else {
                format!("{} ({})", move_found.name, move_found.input)
            }
        };
        CreateEmbed::new()
            .title(title)
            .url(url)
            .field("Damage", move_found.damage, true)
            .field("Guard", move_found.guard, true)
            .field("Startup", move_found.startup, true)
            .field("Active", move_found.active, true)
            .field("Recovery", move_found.recovery, true)
            .field("On Block", move_found.onblock, true)
            .field("Invuln", move_found.invuln, true)
    };
    if move_found.hitboxes.len() == 1 {
        embed = embed.image(move_found.hitboxes.first().unwrap());
        builder = builder.embed(embed);
    } else {
        builder = builder.embed(embed);
        for hitbox in &move_found.hitboxes {
            builder = builder.embed(CreateEmbed::new().image(hitbox).url(url));
        }
    }
    send_reply(ctx, builder).await?;
    Ok(())
}

async fn autocomplete_ggst_character(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<String> {
    let partial = &partial.to_lowercase();
    ctx.data().ggst.read().await.characters.iter()
        .map(|fd| fd.id.to_string())
        .filter(|s| s.to_lowercase().contains(partial))
        .collect::<Vec<String>>()
}

static CHARACTER_ID_MATCHER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"character_query:(\w+)").unwrap());
async fn autocomplete_ggst_move(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<AutocompleteChoice> {
    let partial = &partial.to_lowercase();

    let invocation_string = ctx.invocation_string();
    let option = CHARACTER_ID_MATCHER.captures(&invocation_string).and_then(|c| c.get(1)).map(|c| c.as_str());
    let Some(character_query) = option else {
        return vec![];
    };
    let characters = &ctx.data().ggst.read().await.characters;
    let character_frame_data_opt = characters.iter()
        .find(|c| c.id.to_string().eq_ignore_ascii_case(character_query));
    let Some(character_frame_data) = character_frame_data_opt else {
        return  vec![];
    };
    character_frame_data.moves.iter()
        .filter(|m| m.regex.is_match(partial) || m.name.to_lowercase().starts_with(partial) || m.input.to_lowercase().starts_with(partial))
        .map(|m| AutocompleteChoice::new(format!("{} ({})", m.name, m.input), m.input.to_string()))
        .collect::<Vec<AutocompleteChoice>>()
}