use crate::{app_state::CommandState, bot::utils::BotChats};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all="lowercase")]
pub enum BotType {
  Telegram,
  Discord,
}

#[tauri::command(rename_all="snake_case")]
pub async fn run_bot(
  state: CommandState<'_>,
  token: String,
  system: String,
  model: String,
  allowed_ids: Vec<String>,
  bot_type: BotType
) -> Result<(), ()> {
  if let BotType::Telegram = bot_type { // TELEGRAM BOT =================
    use teloxide::{self as tl, dispatching::{UpdateFilterExt, Dispatcher}};
    use super::telegram_handler::handle_message;

    // validate token
    let bot_status = reqwest::get(format!("https://api.telegram.org/bot{token}/getMe"))
      .await.unwrap().status();

    if bot_status != 200 {
      return Err(());
    }

    // run bot
    let mut dispatcher = Dispatcher::builder(
      tl::Bot::new(&token),
      tl::types::Update::filter_message().endpoint(
        move |bot, chats, msg| handle_message(bot, chats, msg, system.clone(), allowed_ids.clone(), model.clone())
      )
    )
      .dependencies(tl::dptree::deps![BotChats::default()])
      .build();

    state.lock().unwrap().running_tg_bots.push((dispatcher.shutdown_token(), token));

    tauri::async_runtime::spawn(async move {
      dispatcher.dispatch().await;
    });
  } else { // DISCORD BOT ===========================
    use super::discord_handler::{DiscordHandler, BotConfigData, BotConfig};
    use serenity::all::{GatewayIntents as GI, Client};

    // validate token
    let bot_status = reqwest::Client::new().get("https://discord.com/api/v10/users/@me")
      .header("Authorization", format!("Bot {}", token)).send().await.unwrap().status();

    if bot_status != 200 {
      return Err(());
    }

    // run bot
    let mut client = Client::builder(
      &token,
      GI::GUILD_MESSAGES | GI::DIRECT_MESSAGES | GI::MESSAGE_CONTENT
    )
      .event_handler(DiscordHandler).await.unwrap();

    client.data.write().await.insert::<BotConfigData>(BotConfig {
      allowed_ids,
      model,
      system,
      bot_chats: BotChats::default(),
    });

    state.lock().unwrap().running_ds_bots.push((client.shard_manager.clone(), token));

    tauri::async_runtime::spawn(async move {
      client.start().await.unwrap();
    });
  }

  Ok(())
}

#[tauri::command(rename_all="snake_case")]
pub async fn stop_bot(state: CommandState<'_>, bot_type: BotType, token: String) -> Result<(), ()> {
  if let BotType::Telegram = bot_type {
    let running_bots = &mut state.lock().unwrap().running_tg_bots;
    if let Some((shutdown_token, _)) = running_bots.iter().find(|(_, t)| t == &token) {
      std::mem::drop(shutdown_token.shutdown().unwrap());
    }

    running_bots.retain(|(_, t)| t != &token);
  } else {
    let running_bots = &mut state.lock().unwrap().running_ds_bots.clone();
    if let Some((shard_manager, _)) = running_bots.iter().find(|(_, t)| t == &token) {
      tauri::async_runtime::spawn({
        let shard_manager = shard_manager.clone();
        async move {
          shard_manager.shutdown_all().await;
        }
      });
    }

    running_bots.retain(|(_, t)| t != &token);
  }

  Ok(())
}
