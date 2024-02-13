use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use ascii_table::{Align, AsciiTable};
use ggstdl::{GGSTDLData, GGSTDLError};
use rand::prelude::SliceRandom;
use reqwest::Client as HttpClient;
use ruapi::rating::RecentGame;
use serenity::all::standard::macros::hook;
use serenity::async_trait;
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::framework::standard::{Args, CommandResult, Configuration, StandardFramework};
use serenity::framework::standard::macros::{command, group};
use serenity::model::prelude::Message;
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use serenity::Result as SerenityResult;
use songbird::SerenityInit;

use crate::voice::*;

mod voice;

#[group]
#[commands(leave, play, skip, repeat, fd, hb, say, rating, matches, mu, mudata, tierlist)]
struct General;
struct Handler;

const EIGHT_BALL_ANSWERS: [&str; 10] = [
    "It is certain",    "Don’t count on it",
    "It is decidedly so",	"My reply is no",
    "Without a doubt",	"My sources say no",
    "Yes definitely",	"Likely not",
    "Signs point to yes",	"Very doubtful",
];

#[hook]
async fn after(_ctx: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => println!("Processed command '{command_name}'"),
        Err(why) => println!("Command '{command_name}' returned error {why:?}"),
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if let Ok(true) = msg.mentions_me(&ctx.http).await {
            if msg.content.ends_with('?') {
                let text = EIGHT_BALL_ANSWERS.choose(&mut rand::thread_rng()).unwrap_or(&"idk").to_string();
                check_msg(msg.channel_id.say(&ctx.http, text).await);
            }
        }
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        if let Some(id) = old.and_then(|d| d.channel_id) {
            if let Ok(channel) = id.to_channel(&ctx.http).await {
                if let Ok(members) = channel.guild().unwrap().members(&ctx.cache) {
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

struct HttpKey;
impl TypeMapKey for HttpKey {
    type Value = HttpClient;
}

struct GGSTDLCharacterData;
impl TypeMapKey for GGSTDLCharacterData {
    type Value = Arc<RwLock<GGSTDLData>>;
}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new().group(&GENERAL_GROUP);
    framework.configure(Configuration::new().prefix("."));

    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .type_map_insert::<HttpKey>(HttpClient::new())
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

async fn get_http_client(ctx: &Context) -> HttpClient {
    let data = ctx.data.read().await;
    data.get::<HttpKey>()
        .cloned()
        .expect("Guaranteed to exist in the typemap.")
}

#[command]
#[only_in(guilds)]
async fn say(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if args.is_empty() {
        check_msg(msg.channel_id.say(&ctx.http, ".say <message>").await);
        return Ok(());
    }

    let to_say = args.rest();
    if msg.delete(&ctx.http).await.is_ok() {
        let _ = msg.channel_id.say(&ctx.http, to_say).await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn tierlist(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let Ok(matchups) = ruapi::matchup::load_matchups(ruapi::matchup::MatchupChart::TopHundred).await else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not load matchups").await);
        return Ok(());
    };
    let mut tierlist: Vec<(String, f64)> = vec![];
    for (character, mus) in matchups.matchups {
        let count = mus.len() as f64;
        let sum = mus.values().sum::<f64>();
        let avg_winrate = sum / count;
        tierlist.push((character.readablename.clone(), avg_winrate));
    }
    tierlist.sort_by(|(_, b), (_, d)| b.partial_cmp(d).unwrap());
    tierlist.reverse();
    let tierlist = tierlist.into_iter()
        .enumerate()
        .map(|(ind, (c, f))| format!("{}. {} ({:.1}%)", ind + 1, c, f))
        .collect::<Vec<String>>()
        .join("\n");
    check_msg(msg.channel_id.say(&ctx.http, format!("```Tierlist from average winrates:\n{}```", tierlist)).await);
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn mudata(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if args.is_empty() {
        check_msg(msg.channel_id.say(&ctx.http, ".mudata <character>").await);
        return Ok(());
    }
    let Ok(character) = args.single::<String>() else {
        check_msg(msg.channel_id.say(&ctx.http, ".mudata <character>").await);
        return Ok(());
    };
    let Some(character) = ruapi::character::get_character_regex(character) else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find character").await);
        return Ok(());
    };
    let Ok(matchups) = ruapi::matchup::load_matchups(ruapi::matchup::MatchupChart::TopHundred).await else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not load matchups").await);
        return Ok(());
    };
    let Some(winrates) = matchups.matchups.get(character) else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find character winrates").await);
        return Ok(());
    };
    let s = get_mu_data(winrates);
    check_msg(msg.channel_id.say(&ctx.http, s).await);
    Ok(())
}

fn get_mu_data(winrates: &HashMap<&ruapi::character::Character, f64>) -> String {
    let mut winrate_sum = 0.0;
    let mut winrate_count = 0.0;
    let mut winning_sum = 0.0;
    let mut winning_count = 0.0;
    let mut losing_sum = 0.0;
    let mut losing_count = 0.0;
    let mut most_winning_versus: Option<String> = None;
    let mut most_winning = 50.0;
    let mut most_losing_versus: Option<String> = None;
    let mut most_losing = 50.0;
    for (char, winrate) in winrates {
        let winrate = *winrate;
        winrate_sum += winrate;
        winrate_count += 1.0;
        if winrate > 50.0 {
            winning_sum += winrate;
            winning_count += 1.0;
        } else if winrate < 50.0 {
            losing_sum += winrate;
            losing_count += 1.0;
        }
        if winrate > most_winning {
            most_winning = winrate;
            most_winning_versus = Some(char.shortname.clone());
        }
        if winrate < most_losing {
            most_losing = winrate;
            most_losing_versus = Some(char.shortname.clone());
        }
    }
    format!(
        "```Avg Winrate: {:.2}%\nWinning MUs: {} ({}% avg)\nMost Winning: {} ({}%)\nLosing MUs: {} ({}% avg)\nMost Losing: {} ({}%)```",
        (winrate_sum / winrate_count) as usize,
        winning_count as usize, (winning_sum / winning_count) as usize,
        most_winning_versus.unwrap_or("None".to_string()), most_winning,
        losing_count as usize, (losing_sum / losing_count) as usize,
        most_losing_versus.unwrap_or("None".to_string()), most_losing
    )
}

#[command]
#[only_in(guilds)]
async fn mu(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if args.len() < 2 {
        check_msg(msg.channel_id.say(&ctx.http, ".mu <character> <versus>").await);
        return Ok(());
    }
    let Ok(character) = args.single::<String>() else {
        check_msg(msg.channel_id.say(&ctx.http, ".mu <character> <versus>").await);
        return Ok(());
    };
    let Ok(versus) = args.single::<String>() else {
        check_msg(msg.channel_id.say(&ctx.http, ".mu <character> <versus>").await);
        return Ok(());
    };
    let Some(character) = ruapi::character::get_character_regex(character) else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find character").await);
        return Ok(());
    };
    let Some(versus) = ruapi::character::get_character_regex(versus) else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find versus character").await);
        return Ok(());
    };
    let Ok(matchups) = ruapi::matchup::load_matchups(ruapi::matchup::MatchupChart::TopHundred).await else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not load matchups").await);
        return Ok(());
    };
    let Some(matchup) = matchups.get_matchup(character, versus) else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find matchup").await);
        return Ok(());
    };
    let formatted = format!("{} vs {}: {}%", character.shortname, versus.shortname, matchup);
    check_msg(msg.channel_id.say(&ctx.http, formatted).await);
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn matches(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if args.len() < 2 {
        check_msg(msg.channel_id.say(&ctx.http, ".matches <player> <character>").await);
        return Ok(());
    }
    let Ok(name_query) = args.single::<String>() else {
        check_msg(msg.channel_id.say(&ctx.http, ".matches <player> <character>").await);
        return Ok(());
    };
    let Some(character_query) = args.remains() else {
        check_msg(msg.channel_id.say(&ctx.http, ".matches <player> <character>").await);
        return Ok(());
    };
    let Some(character) = ruapi::character::get_character_regex(character_query.to_string()) else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find character").await);
        return Ok(());
    };
    let Ok(player_data) = ruapi::rating::player_lookup_character(&name_query, character).await else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find player, or player with that character. Names must be exact.").await);
        return Ok(());
    };
    let Ok(recent_games) = ruapi::rating::load_match_history_id(&player_data.id, character).await else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not load recent games").await);
        return Ok(());
    };
    let table = generate_table(recent_games);
    let full_str = format!("```Rating: {} ± {} ({} games)\n{}```",
                           player_data.character.rating,
                           player_data.character.deviation,
                           player_data.character.game_count,
                           table);
    check_msg(msg.channel_id.say(&ctx.http, full_str).await);
    return Ok(());
}

#[command]
#[only_in(guilds)]
async fn rating(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if args.len() < 2 {
        check_msg(msg.channel_id.say(&ctx.http, ".rating <player> <character>").await);
        return Ok(());
    }

    // grab all args up until the last one. (some names have spaces)
    let mut name_query: Vec<String> = vec![];
    for _ in 0..(args.len() - 1) {
        name_query.push(args.single::<String>().unwrap());
    }
    let name_query = name_query.join(" ");

    let Some(character_query) = args.remains() else {
        check_msg(msg.channel_id.say(&ctx.http, ".rating <player> <character>").await);
        return Ok(());
    };
    let Some(character) = ruapi::character::get_character_regex(character_query.to_string()) else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find character").await);
        return Ok(());
    };
    let Ok(player_data) = ruapi::rating::player_lookup_character(&name_query, character).await else {
        check_msg(msg.channel_id.say(&ctx.http, "Could not find player, or player with that character. Names must be exact.").await);
        return Ok(());
    };
    let full_str = format!("```{}'s rating: {} ± {} ({} games)```",
        name_query,
        player_data.character.rating,
        player_data.character.deviation,
        player_data.character.game_count);
    check_msg(msg.channel_id.say(&ctx.http, full_str).await);
    return Ok(());
}

fn generate_table(recent_games: Vec<RecentGame>) -> String {
    let table = get_table_template();
    let mut data: Vec<Vec<String>> = vec![];
    for game in recent_games.into_iter().take(5) {
        data.push(vec![game.rating, game.floor,
                       format!("{} ({})", game.opponent, game.opponent_character),
                       game.opponent_rating, game.odds, game.result, game.rating_change]);
    }
    table.format(data)
}

fn get_table_template() -> AsciiTable {
    let mut table = AsciiTable::default();
    table.column(0).set_header("Rating").set_align(Align::Center);
    table.column(1).set_header("Floor").set_align(Align::Center);
    table.column(2).set_header("Opponent").set_align(Align::Center);
    table.column(3).set_header("Rating").set_align(Align::Center);
    table.column(4).set_header("Odds").set_align(Align::Center);
    table.column(5).set_header("Result").set_align(Align::Center);
    table.column(6).set_header("Change").set_align(Align::Center);
    table
}

#[command]
#[only_in(guilds)]
async fn hb(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
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
    let text = if move_found.hitboxes.is_empty() {
        String::from("none")
    } else {
        move_found.hitboxes.join(" \n ")
    };
    check_msg(msg.channel_id.say(&ctx.http, text).await);
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

    let embed = {
        let title = {
            if move_found.input.eq_ignore_ascii_case(&move_found.name) {
                move_found.input
            } else {
                format!("{} ({})", move_found.name, move_found.input)
            }
        };
        let mut em = CreateEmbed::new()
            .title(title)
            .field("Damage", move_found.damage, true)
            .field("Guard", move_found.guard, true)
            .field("Startup", move_found.startup, true)
            .field("Active", move_found.active, true)
            .field("Recovery", move_found.recovery, true)
            .field("On Block", move_found.onblock, true)
            .field("Invuln", move_found.invuln, true);
        if let Some(first) = move_found.hitboxes.first() {
            em = em.image(first);
        }
        em
    };
    let builder = CreateMessage::new().embed(embed);
    let v = msg.channel_id.send_message(&ctx.http, builder).await;
    check_msg(v);

    Ok(())
}

// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}