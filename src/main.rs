mod tasks {
    pub(crate) mod text_generation;
    pub(crate) mod handle_errors;
    pub(crate) mod image_generation;
    pub(crate) mod misc_commands;
}

use std::{env, sync::Arc, time::Duration};

use poise::serenity_prelude as serenity;

use serenity::{
    async_trait, builder::{CreateAllowedMentions, CreateMessage}, http::Typing, model::{channel::Message, gateway::Ready}, prelude::*
};

use tokio::task;
use tasks::{handle_errors::return_error_reply, image_generation::imagegen, misc_commands::help, text_generation::text_reply};

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

struct Handler;

/*
    This EventHandler is used by serenity to process any events that happen
    This program currently supports
        - message - for when any messages are recieved wherever the bot has access to messages (includes DMs)
        - ready - for when the bot has successfully connected to Discord
*/
#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: serenity::prelude::Context, msg: Message) {
        task::spawn(async move {
            let author_id: String = env::var("USER_ID").unwrap_or("-1".to_owned());
            let debug_enabled: String = env::var("DEBUG").unwrap_or("0".to_owned());

            if debug_enabled != "1".to_owned() ||
            author_id == msg.author.id.to_string() {
                let message_prefix: String;
                if debug_enabled == "1".to_owned() {
                    message_prefix = "DEBUG: ".to_string();
                } else {
                    message_prefix = "".to_string();
                }
                /*
                    This if statement splits between a text reply and using a function from functions.json
                 */
                if msg.author.id != ctx.cache.current_user().id {
                    if msg.mentions_user_id(ctx.cache.current_user().id) {
                        let http_cache = ctx.clone().http;
                        let current_user_id: u64 = ctx.cache.current_user().id.into();
                        let typing = Typing::start(http_cache.clone(), msg.channel_id.into());
                        let response_vec = text_reply(msg.clone(), &ctx, current_user_id).await;
                        for response in response_vec {
                            let message_builder = CreateMessage::new()
                                .reference_message(&msg)
                                .allowed_mentions(CreateAllowedMentions::new().users(vec![msg.clone().author.id]))
                                .content(format!("{}{}", message_prefix, response));
                            match msg.channel_id.send_message(http_cache.clone(), message_builder).await
                            {
                                Ok(t) => t,
                                Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        }
                        typing.stop();

                    }
                }
            }
        });
    }

    async fn ready(&self, _: serenity::prelude::Context, ready: Ready) {
            println!("{} is connected!", ready.user.name);
    }
}
#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment");

    let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::MESSAGE_CONTENT;

    let framework_options = poise::FrameworkOptions { 
        commands: vec![
            imagegen(),
            help()
        ],
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
