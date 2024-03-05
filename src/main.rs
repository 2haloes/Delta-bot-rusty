mod tasks {
    pub(crate) mod text_generation;
    pub(crate) mod handle_errors;
    pub(crate) mod image_generation;
}

use std::{borrow::Borrow, env, sync::Arc, time::Duration};

use poise::serenity_prelude as serenity;

use serenity::{
    async_trait, builder::{CreateAllowedMentions, CreateAttachment, CreateMessage}, http::Typing, model::{channel::Message, gateway::Ready}, prelude::*
};

use tokio::task;
use tasks::{handle_errors::{on_error, return_error_reply}, image_generation::{generate_dalle, generate_runpod_image, imagegen}, text_generation::text_reply};

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
                    if msg.content.starts_with("!delta") {
                        let typing = Typing::start(ctx.clone().http, msg.channel_id.into());
                        let msg_safe = msg.borrow().content_safe(ctx.clone().cache);
                        let mut full_command = msg_safe.splitn(2, " ");
                        let command_string = match full_command.next()
                            {
                                Some(t) => t,
                                None => return_error_reply(msg.clone(), "Unable to process command string".to_owned()).await.unwrap(),
                            };
                        let command_data = match full_command.next()
                            {
                                Some(t) => t,
                                None => return_error_reply(msg.clone(), "Unable to process command data string".to_owned()).await.unwrap(),
                            };
                        let function_json_string = match tokio::fs::read_to_string("assets/functions.json").await
                            {
                                Ok(t) => t,
                                Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        let function_object: JsonObject = match serde_json::from_str(&function_json_string)
                            {
                                Ok(t) => t,
                                Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        let current_function: FunctionData = match function_object.function_data.into_iter()
                            .filter(|function| function.function_command == command_string).next()
                            {
                                Some(t) => t,
                                None => return_error_reply(msg.clone(), "Unable to process current function string".to_owned()).await.unwrap(),
                            };
                        let command_function_str = current_function.function_type.as_str();
                        let command_api_str = current_function.function_api_key;
                        let command_data_prefix = current_function.prompt_prefix;
                        let command_data_suffix = current_function.prompt_suffix;
                        let image_attachments: Vec<CreateAttachment>;
                        let full_command_string: String = format!("{command_data_prefix}{command_data}{command_data_suffix}");

                        /*
                            Ths chooses which functionality needs to be used based on the command provided
                            Currently this supports generating images from DALL-E or a Runpod serverless instance
                        */
                        match command_function_str {
                            // "openai_dalle"=>{
                            //     image_attachments = generate_dalle(full_command_string, msg.clone()).await;
                            // }
                            // "runpod_image"=>{
                            //     image_attachments = generate_runpod_image(full_command_string, command_api_str, msg.clone()).await;
                            // }
                            _=>{
                                // If a reply can't be made... is there a point in trying to reply with a different error?
                                msg.reply(ctx.http, format!("{}Your command has not been recognised, sorry I couldn't help!", message_prefix)).await.expect("Unable to send default command reply");
                                return;
                            }
                        }

                        /*
                            Construct a message using the Message builder
                            Refrences the orginal message sent to reply to it
                            Creates an allowed mention for the orginal poster 
                            Attaches the attachments generated in the above function call
                            And adds a bit of text
                        */
                        let message_builder = CreateMessage::new()
                            .reference_message(&msg)
                            .allowed_mentions(CreateAllowedMentions::new().users(vec![msg.clone().author.id]))
                            .files(image_attachments)
                            .content(format!("{}Hello, thank you for the request! Here is the image you've requested!", message_prefix));
                        
                        match msg.channel_id.send_message(ctx.http, message_builder).await
                            {
                                Ok(t) => t,
                                Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        typing.stop();
                    } else if msg.mentions_user_id(ctx.cache.current_user().id) {
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
        commands: vec![imagegen()],
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
        // The global error handler for all error cases that may occur
        on_error: |error| Box::pin(on_error(error)),
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
        // Every command invocation must pass this check to continue execution
        command_check: Some(|ctx| {
            Box::pin(async move {
                if ctx.author().id == 123456789 {
                    return Ok(false);
                }
                Ok(true)
            })
        }),
        // Enforce command checks even for owners (enforced by default)
        // Set to true to bypass checks, which is useful for testing
        skip_checks_for_owners: false,
        event_handler: |_ctx, event, _framework, _data| {
            Box::pin(async move {
                println!(
                    "Got an event in event handler: {:?}",
                    event.snake_case_name()
                );
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
