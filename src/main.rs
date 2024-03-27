mod tasks {
    pub(crate) mod text_generation;
    pub(crate) mod handle_errors;
    pub(crate) mod image_generation;
    pub(crate) mod misc_commands;
    pub(crate) mod tts;
    pub(crate) mod stt;
    pub(crate) mod ffmpeg_handler;
}

use std::{env, path::PathBuf, sync::Arc, time::Duration};

use poise::serenity_prelude as serenity;

use ::serenity::all::FullEvent;
use serenity::{
    builder::{CreateAllowedMentions, CreateMessage}, http::Typing, model::channel::Message, prelude::*
};

use tasks::{handle_errors::return_error_reply, image_generation::imagegen, misc_commands::help, stt::{transcribe_from_attachment, transcribe_from_message, transcribe_from_url}, text_generation::text_reply, tts::{tts_from_message, tts_from_text}};

use which::which;

struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(serde::Deserialize)]
#[derive(Clone)]
struct FunctionData {
    function_command: String,
    function_type: String,
    function_api_key: String,
    function_friendly_name: String,
    prompt_prefix: String,
    prompt_suffix: String
}

#[derive(serde::Deserialize)]
struct JsonObject{
    function_data: Vec<FunctionData>
}

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment");

    let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::MESSAGE_CONTENT;

    let mut command_set = vec![
        imagegen(),
        help()
    ];

    // Check that FFmpeg is installed and avaliable, this is needed for media conversions
    let result_test = which("ffmpeg").unwrap_or(PathBuf::default());

    // If FFmpeg is avaliable, add the commands that depend on it to the commands list
    if result_test != PathBuf::default() {
        command_set.push(tts_from_text());
        command_set.push(tts_from_message());
        command_set.push(transcribe_from_attachment());
        command_set.push(transcribe_from_message());
        command_set.push(transcribe_from_url())
    }


    let framework_options = poise::FrameworkOptions { 
        commands: command_set,
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some("!delta".into()),
            edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                Duration::from_secs(3600),
            ))),
            additional_prefixes: vec![
                poise::Prefix::Literal("!Delta"),
                poise::Prefix::Literal("delta"),
                poise::Prefix::Literal("Delta"),
            ],
            ..Default::default()
        },
        // This code is run before every command
        pre_command: |ctx| {
            Box::pin(async move {
                println!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        // This code is run after a command if it was successful (returned Ok)
        post_command: |ctx| {
            Box::pin(async move {
                println!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        /*
            This EventHandler is used by serenity to process any events that happen
            This program currently supports
                - message - for when any messages are recieved wherever the bot has access to messages (includes DMs)
        */
        event_handler: |ctx, event, _framework, _data| {
            Box::pin(async move {
                match event {
                    FullEvent::Message { new_message } => {
                        let author_id: String = env::var("USER_ID").unwrap_or("-1".to_owned());
                        let debug_enabled: String = env::var("DEBUG").unwrap_or("0".to_owned());

                        if debug_enabled != "1".to_owned() ||
                        author_id == new_message.author.id.to_string() {
                            let message_prefix: String;
                            if debug_enabled == "1".to_owned() {
                                message_prefix = "DEBUG: ".to_string();
                            } else {
                                message_prefix = "".to_string();
                            }
                            if new_message.author.id != ctx.cache.current_user().id && new_message.mentions_user_id(ctx.cache.current_user().id) {
                                let http_cache = ctx.clone().http;
                                let current_user_id: u64 = ctx.cache.current_user().id.into();
                                let typing = Typing::start(http_cache.clone(), new_message.channel_id.into());
                                let response_vec = text_reply(new_message.clone(), &ctx, current_user_id).await;
                                let mut last_sent_reply = Message::default();
                                let _default_message = Message::default();
                                for response in response_vec {
                                    let mut response_message: &Message = new_message;
                                    // The message ID is set to 1 if it is default, I am making an assumption that this bot will never get a new message with the ID of 1
                                    // Even if this was the case, this will only affect multi message responses
                                    if last_sent_reply.id != 1 {
                                        response_message = &last_sent_reply;
                                    }
                                    let message_builder = CreateMessage::new()
                                        .reference_message(response_message)
                                        .allowed_mentions(CreateAllowedMentions::new().users(vec![new_message.clone().author.id]))
                                        .content(format!("{}{}", message_prefix, response));
                                    last_sent_reply = match new_message.channel_id.send_message(http_cache.clone(), message_builder).await
                                    {
                                        Ok(t) => t,
                                        Err(e) => return_error_reply(new_message.clone(), e.to_string()).await.unwrap(),
                                    };
                                }
                                typing.stop();
                            }
                        }
                    }
                    _ => {}
                }
                Ok(())
            })
        },
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .options(framework_options)
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
