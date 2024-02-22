mod tasks {
    pub(crate) mod text_generation;
    pub(crate) mod handle_errors;
    pub(crate) mod image_generation;
}

use std::{borrow::Borrow, env};

use serenity::{
    async_trait, builder::{CreateAllowedMentions, CreateAttachment, CreateMessage}, http::Typing, model::{channel::Message, gateway::Ready}, prelude::*
};

use tokio::task;
use tasks::{handle_errors::return_error, image_generation::{generate_dalle, generate_runpod_image}, text_generation::text_reply};

#[derive(serde::Deserialize)]
struct FunctionData {
    function_command: String,
    function_type: String,
    function_api_key: String,
    prompt_prefix: String,
    prompt_suffix: String
}

#[derive(serde::Deserialize)]
struct JsonObject{
    function_data: Vec<FunctionData>
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
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
                if msg.author.id != ctx.cache.current_user().id {
                    if msg.content.starts_with("!delta") {
                        let typing = Typing::start(ctx.clone().http, msg.channel_id.into());
                        let msg_safe = msg.borrow().content_safe(ctx.clone().cache);
                        let mut full_command = msg_safe.splitn(2, " ");
                        let command_string = match full_command.next()
                            {
                                Some(t) => t,
                                None => return_error(msg.clone(), "Unable to process command string".to_owned()).await.unwrap(),
                            };
                        let command_data = match full_command.next()
                            {
                                Some(t) => t,
                                None => return_error(msg.clone(), "Unable to process command data string".to_owned()).await.unwrap(),
                            };
                        let function_json_string = match tokio::fs::read_to_string("assets/functions.json").await
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        let function_object: JsonObject = match serde_json::from_str(&function_json_string)
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        let current_function: FunctionData = match function_object.function_data.into_iter()
                            .filter(|function| function.function_command == command_string).next()
                            {
                                Some(t) => t,
                                None => return_error(msg.clone(), "Unable to process current function string".to_owned()).await.unwrap(),
                            };
                        let command_function_str = current_function.function_type.as_str();
                        let command_api_str = current_function.function_api_key;
                        let command_data_prefix = current_function.prompt_prefix;
                        let command_data_suffix = current_function.prompt_suffix;
                        let image_attachments: Vec<CreateAttachment>;
                        let full_command_string: String = format!("{command_data_prefix}{command_data}{command_data_suffix}");

                        match command_function_str {
                            "openai_dalle"=>{
                                image_attachments = generate_dalle(full_command_string, msg.clone()).await;
                            }
                            "runpod_image"=>{
                                image_attachments = generate_runpod_image(full_command_string, command_api_str, msg.clone()).await;
                            }
                            _=>{
                                // If a reply can't be made... is there a point in trying to reply with a different error?
                                msg.reply(ctx.http, format!("{}Your command has not been recognised, sorry I couldn't help!", message_prefix)).await.expect("Unable to send default command reply");
                                return;
                            }
                        }

                        let message_builder = CreateMessage::new()
                            .reference_message(&msg)
                            .allowed_mentions(CreateAllowedMentions::new().users(vec![msg.clone().author.id]))
                            .files(image_attachments)
                            .content(format!("{}Hello, thank you for the request! Here is the image you've requested!", message_prefix));
                        
                        match msg.channel_id.send_message(ctx.http, message_builder).await
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
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
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        }
                        typing.stop();

                    }
                }
            }
        });
    }

    async fn ready(&self, _: Context, ready: Ready) {
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

    let mut client = serenity::Client::builder(&token, intents).event_handler(Handler).await.expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
