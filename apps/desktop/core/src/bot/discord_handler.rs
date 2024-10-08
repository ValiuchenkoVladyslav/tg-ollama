use super::utils::{get_current_time, BotConfig, BATCHING_MILLIS};
use crate::ollama::api::{ChatStream, OllamaMessage, Role};
use futures::StreamExt;
use serenity::all::{async_trait, Context, EditMessage, EventHandler, Message};

pub struct BotConfigData;

impl serenity::prelude::TypeMapKey for BotConfigData {
  type Value = BotConfig;
}

pub struct DiscordHandler;

#[async_trait]
impl EventHandler for DiscordHandler {
  async fn message(&self, ctx: Context, msg: Message) {
    if msg.content.is_empty() {
      return; // Ignore non-text messages
    }

    let bot_data = ctx.data.read().await;
    let BotConfig {
      allowed_ids,
      model,
      system,
      bot_chats,
    } = bot_data.get::<BotConfigData>().unwrap();

    let chat_id = msg.channel_id.to_string();

    if !allowed_ids.contains(&msg.author.id.to_string()) {
      return; // Ignore messages from not allowed users
    }

    let mut message_history = match bot_chats
      .lock()
      .unwrap()
      .iter()
      .find(|(id, _)| id == &chat_id)
    {
      Some(chat) => chat.1.clone(),
      None => vec![OllamaMessage {
        role: Role::System,
        content: system.into(),
      }],
    };

    message_history.push(OllamaMessage {
      role: Role::User,
      content: msg.content.clone(),
    });

    // Get response from Ollama and send it to discord
    let Ok(mut res_stream) = ChatStream::new(&message_history, model).await else {
      msg
        .reply(&ctx.http, "ERROR: Failed to connect to Ollama server!")
        .await
        .unwrap();
      return;
    };

    let mut ai_response = res_stream.next().await.unwrap().message;
    let mut bot_msg = msg.reply(&ctx.http, &ai_response.content).await.unwrap();

    let mut start_time = get_current_time();
    while let Some(res) = res_stream.next().await {
      ai_response.content.push_str(&res.message.content);
      let current_time = get_current_time();

      // in order to avoid telegram rate limits
      if current_time - start_time > std::time::Duration::from_millis(BATCHING_MILLIS * 2) {
        let _ = bot_msg
          .edit(&ctx.http, EditMessage::new().content(&ai_response.content))
          .await;
        start_time = current_time;
      }
    }

    if start_time.as_millis() % (BATCHING_MILLIS * 2) as u128 != 0 {
      // append missing final part if it exists
      let _ = bot_msg
        .edit(&ctx.http, EditMessage::new().content(&ai_response.content))
        .await;
    }

    // Save new chat messages
    let mut bot_chats = bot_chats.lock().unwrap();

    message_history.push(ai_response);

    if let Some(chat_index) = bot_chats.iter().position(|(id, _)| id == &chat_id) {
      bot_chats[chat_index].1 = message_history;
    } else {
      bot_chats.push((chat_id, message_history));
    }
  }
}
