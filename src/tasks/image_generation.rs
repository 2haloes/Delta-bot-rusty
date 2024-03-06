use std::{env, fs, sync::Arc, time::Duration, u32};

use poise::serenity_prelude as serenity;
use async_openai::{types::{CreateImageRequestArgs, Image, ImageModel, ImageQuality, ImageSize, ImageStyle, ResponseFormat}, Client};
use base64::prelude::*;
use reqwest::{header::HeaderValue, Response};
use ::serenity::all::{ComponentInteractionDataKind, CreateAllowedMentions, CreateEmbed, CreateMessage, CreateSelectMenuOption, Typing};
use serenity::all::CreateAttachment;
use tokio::time::sleep;

use crate::{tasks::handle_errors::return_error, Error, FunctionData, JsonObject};


#[derive(serde::Deserialize)]
struct RunResponseObject {
    id: String,
}

#[derive(serde::Deserialize)]
struct ImageGenOutput {
    images: Vec<String>,
}

#[derive(serde::Deserialize)]
struct OutputResponseObject {
    output: Option<ImageGenOutput>,
    status: Option<String>
}

#[derive(serde::Serialize)]
struct ImageGenRunInput {
    prompt: String,
    negative_prompt: String,
    width: u32,
    height: u32,
    init_image: String,
    mask: String,
    guidance_scale: f32,
    num_inference_steps: u32,
    num_outputs: u32,
    prompt_strength: f32,
    scheduler: String,
    seed: u32
}

#[derive(serde::Serialize)]
struct ImageGenRequest {
    input: ImageGenRunInput,
    webhook: String
}

#[derive(Debug, poise::Modal)]
#[name = "Runpod Generation"]
struct ServerlessModal {
    #[name = "Prompt"]
    prompt: String,
    #[name = "Negative Prompt (Default: null)"]
    neg_prompt: Option<String>,
    #[name = "Width Ratio (Default: 1)"]
    width_ratio: Option<String>,
    #[name = "Height Ratio (Default: 1)"]
    height_ratio: Option<String>,
    #[name = "Guidance scale (Default: 7.5)"]
    guide_scale: Option<String>,
}

#[derive(Debug, poise::Modal)]
#[name = "DALL-E Generation"]
struct DalleModal {
    #[name = "Prompt"]
    prompt: String,}

#[poise::command(prefix_command, slash_command)]
pub async fn imagegen(ctx: crate::Context<'_>) -> Result<(), Error> {

    //let bug_message = Message::default();
    let requester_id = ctx.author().id;
    let channel_id = ctx.channel_id();

    let function_json_string = match fs::read_to_string("assets/functions.json")
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
    let function_object: JsonObject = match serde_json::from_str(&function_json_string)
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
    let function_data = function_object.function_data;

    let mut model_options: Vec<CreateSelectMenuOption> = Vec::new();
    
    for command_object in function_data.clone().into_iter() {
        model_options.push(CreateSelectMenuOption::new(command_object.function_friendly_name, command_object.function_command));
    };

    let reply = {
        let components: Vec<serenity::CreateActionRow> = vec![poise::serenity_prelude::CreateActionRow::SelectMenu(serenity::CreateSelectMenu::new("model_select",
            serenity::builder::CreateSelectMenuKind::String { options: model_options }
    ))];

        poise::CreateReply::default()
            .content("Please select which model to use")
            .components(components)
    };

    let sent_message = ctx.send(reply).await?;

    while let Some(mci) = serenity::ComponentInteractionCollector::new(ctx.serenity_context())
        .timeout(std::time::Duration::from_secs(120))
        .filter(move |mci| mci.data.custom_id == "model_select")
        .await
    {
        // Cannot get ctx to be used with typing, requires Arc<Http> when the ctx only returns &Http
        let typing_cache = serenity::Http::new(&env::var("DISCORD_TOKEN").expect("Expected a token in the environment"));
        let typing_cache_arc: Arc<serenity::Http> = Arc::new(typing_cache);
        let typing: Typing;
        let data_kind = mci.clone().data.kind;
        let current_command = match data_kind {
            ComponentInteractionDataKind::StringSelect { values } => {values[0].clone()},
            _ => return_error(requester_id.clone(), channel_id.clone(), "An invalid response has been returned from the dropdown".to_owned()).await.unwrap()
        };
    
        let current_function: FunctionData = match function_data.clone().into_iter()
            .filter(|function| function.function_command == current_command).next()
            {
                Some(t) => t,
                None => return_error(requester_id.clone(), channel_id.clone(), "Unable to process current function string".to_owned()).await.unwrap(),
            };
        let command_function_type = current_function.function_type.as_str();
        let command_api_str = current_function.function_api_key;
        let command_data_prefix = current_function.prompt_prefix;
        let command_data_suffix = current_function.prompt_suffix;
        let full_message = sent_message.clone().into_message().await?;
        let _message_deleted = full_message.delete(ctx).await?;
        let image_attachments: Vec<CreateAttachment>;
        let mut embed_set: Vec<CreateEmbed> = Vec::new();
        let prompt: String;

        match command_function_type {
            "runpod_image" => {
                typing = Typing::start(typing_cache_arc, channel_id.into());
                let data =
                    poise::execute_modal_on_component_interaction::<ServerlessModal>(ctx, mci.clone(), None, None).await?;
                let data_unwrapped = data.unwrap();
                let width_ratio: f32 = match data_unwrapped.width_ratio.unwrap_or("1".to_string()).parse()
                {
                    Ok(t) => t,
                    Err(_) => return_error(requester_id.clone(), channel_id.clone(), "Non number entered into width ratio field".to_owned()).await.unwrap(),
                };
                let height_ratio: f32 = match data_unwrapped.height_ratio.unwrap_or("1".to_string()).parse()
                {
                    Ok(t) => t,
                    Err(_) => return_error(requester_id.clone(), channel_id.clone(), "Non number entered into height ratio field".to_owned()).await.unwrap(),
                };
                prompt = data_unwrapped.prompt;
                let neg_prompt: String = data_unwrapped.neg_prompt.unwrap_or("".to_string());
                let guide_scale: f32 = match data_unwrapped.guide_scale.unwrap_or("7.5".to_string()).parse()
                {
                    Ok(t) => t,
                    Err(_) => return_error(requester_id.clone(), channel_id.clone(), "Non number entered into height ratio field".to_owned()).await.unwrap(),
                };
                // This is a fixed value from 1024*1024 (this being the default SDXL height and width)
                let total_pixel_count: f32 = 1048576.0;
                // Calculate the image size based on the aspect ratio and total number of pixels the model allows
                // For example, Stable Diffusion XL supports 1024x1024 so the total pixesl would be the result of 1024*1024
                let height: f32 = ((((total_pixel_count * (height_ratio / width_ratio)).sqrt()).round()as u32) + 7 & !7) as f32;
                let width: f32 = ((((width_ratio / height_ratio) * height).round() as u32) + 7 & !7) as f32;
                let full_prompt = format!("{}{}{}", command_data_prefix, prompt, command_data_suffix);
                
                image_attachments = generate_runpod_image(full_prompt, command_api_str, width, height, 2, neg_prompt.clone(), guide_scale, ctx).await;

                // First image is pushed with the embed, this is because the content of the embed is dependent on the model selected
                embed_set.push(
                    CreateEmbed::new()
                        .attachment(image_attachments[0].clone().filename)
                        .url("https://runpod.io")
                        .description(
                            format!(
                                "Congratulations <@{}>, your image has been generated with the following input\n\n> Model: {}\n> Prompt: {}\n> Neg prompt: {}\n> Width ratio: {} (Actual width: {})\n> Height ratio: {} (Actual height: {})\n> Guidance scale: {}", 
                                requester_id,
                                current_command,
                                prompt,
                                neg_prompt,
                                width_ratio,
                                width,
                                height_ratio,
                                height,
                                guide_scale
                            )
                        )
                );
            },
            "openai_dalle" => {
                typing = Typing::start(typing_cache_arc, channel_id.into());
                let data =
                    poise::execute_modal_on_component_interaction::<DalleModal>(ctx, mci.clone(), None, None).await?;
                let data_unwrapped = data.unwrap();
                let prompt = data_unwrapped.prompt;
                // No option for multiple generations at one time with DALL-E 3
                image_attachments = generate_dalle(prompt.clone(), ctx).await;

                embed_set.push(
                    CreateEmbed::new()
                        .attachment(image_attachments[0].clone().filename)
                        .url("https://runpod.io")
                        .description(
                            format!(
                                "Congratulations <@{}>, your image has been generated with the following input\n\n> Model: {}\n> Prompt: {}", 
                                requester_id,
                                current_command,
                                prompt
                            )
                        )
                );
            }
            _ => {panic!("Oh noes!");}
        }
        
        for image_attach in image_attachments.clone().into_iter().skip(1) {
            embed_set.push(
                CreateEmbed::new()
                    .url("https://runpod.io")
                    .attachment(image_attach.filename)
            );
        };

        let message_builder = CreateMessage::new()
            .allowed_mentions(CreateAllowedMentions::new().users(vec![requester_id]))
            .content(format!("<@{}>", requester_id))
            .files(image_attachments)
            .add_embeds(embed_set);
        
        match channel_id.send_message(ctx, message_builder).await
            {
                Ok(t) => t,
                Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
            };
        typing.stop();
        
    }
    Ok(())
}

/*
    This generates images using DALL-E
    It uses the openai-async library for making calls
*/
async fn generate_dalle (prompt_text: String, ctx: crate::Context<'_>) -> Vec<CreateAttachment> {
    let client = Client::new();
    let requester_id = ctx.author().id;
    let channel_id = ctx.channel_id();
    let request = match CreateImageRequestArgs::default()
        .prompt(prompt_text)
        .n(1)
        .response_format(ResponseFormat::B64Json)
        .size(ImageSize::S1024x1024)
        .user("Delta-Bot")
        .model(ImageModel::DallE3)
        .quality(ImageQuality::HD)
        .style(ImageStyle::Vivid)
        .build()
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };

    let response = match client.images().create(request).await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
    let mut image_attachments: Vec<CreateAttachment> = Vec::default();

    /*
        This loop goes through every image in the reply and converts it from base64 to bytes
        This is then set as an attachment for a Discord message
    */
    for (index, image_data) in response.data.iter().enumerate() {
        let mut image_data_base_64 = "".to_owned();

        match &**image_data {
            Image::B64Json {b64_json, revised_prompt: _} => {
                image_data_base_64 = b64_json.as_str().to_owned();
            },
            Image::Url {..} => {
                return_error(requester_id.clone(), channel_id.clone(), "Expected Base64 from DALL-E, got a different output instead".to_owned()).await.unwrap()
            }
        }
        let base64_image_cleaned = image_data_base_64.replace("data:image/png;base64,", "");
        match BASE64_STANDARD.decode(&base64_image_cleaned) {
            Ok(bytes) => image_attachments.push(CreateAttachment::bytes(bytes, format!("image_output_{index}.png"))),
            Err(err) => {
                println!("At least one image returned an exception/n{}", err);
            }
        }
    }

    return image_attachments;
}

/*
    This generates images using Runpod serverless
    This uses reqwest to call the API
    Note that currently, the serverless implimentation must return a base64 string
    This should work with any Stable Diffusion/Stable Diffusion XL endpoint that is based on the offical API
*/
async fn generate_runpod_image (
    prompt_text: String, 
    model_ref: String, 
    width: f32, 
    height: f32, 
    num_gen: u32,
    neg_prompt: String,
    guide_scale: f32,
    ctx: crate::Context<'_>
) -> Vec<CreateAttachment> {
    let requester_id = ctx.author().id;
    let channel_id = ctx.channel_id();
    let runpod_auth_key = match env::var("RUNPOD_API_KEY")
        {
            Ok(t) => t,
            Err(_) => return_error(requester_id.clone(), channel_id.clone(), "No runpod API key found".to_owned()).await.unwrap(),
        };
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("accept", HeaderValue::from_static("application/json"));
    headers.insert("authorization", HeaderValue::from_str(&runpod_auth_key).unwrap());
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    let client = reqwest::Client::new();
    let run_response: Response = match client.post(format!("https://api.runpod.ai/v2/{}/run", model_ref))
        .headers(headers.clone())
        .body(format!("{{
            \"input\": {{
                \"prompt\": \"{}\",
                \"negative_prompt\": \"{}\",
                \"height\": {},
                \"width\": {},
                \"scheduler\": \"K_EULER\",
                \"num_inference_steps\": 40,
                \"guidance_scale\": {},
                \"num_images\": {}
            }}
        }}", prompt_text, neg_prompt, height, width, guide_scale, num_gen))
        .send()
        .await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
       

    let run_response_json: RunResponseObject = match run_response.json()
        .await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };

    let mut status_response_json: OutputResponseObject;
    
    /*
        This calls the job status endpoint every 2 seconds
        When the status changes from IN_QUEUE and IN_PROGRESS, the result is then used to get the image information
    */
    loop {
        let status_response = match client.post(format!("https://api.runpod.ai/v2/{}/status/{}", model_ref, run_response_json.id))
            .headers(headers.clone())
            .send()
            .await
            {
                Ok(t) => t,
                Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
            };
        

        status_response_json = match status_response.json()
            .await
            {
                Ok(t) => t,
                Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
            };

        if !(status_response_json.status.clone().unwrap() == "IN_QUEUE" || status_response_json.status.clone().unwrap() == "IN_PROGRESS") {
            break;
        }

        let _ = sleep(Duration::from_secs(2));
    }

    let mut image_attachments: Vec<CreateAttachment> = Vec::default();

    let image_output = match status_response_json.output
    {
        Some(t) => t,
        None => return_error(requester_id.clone(), channel_id.clone(), "No generated image data found".to_owned()).await.unwrap(),
    };

    /*
        This loop goes through every image in the reply and converts it from base64 to bytes
        This is then set as an attachment for a Discord message
    */
    for (index, base64_image) in image_output.images.iter().enumerate() {
        let base64_image_cleaned = base64_image.replace("data:image/png;base64,", "");
        match BASE64_STANDARD.decode(&base64_image_cleaned) {
            Ok(bytes) => image_attachments.push(CreateAttachment::bytes(bytes, format!("image_output_{index}.png"))),
            Err(err) => {
                println!("At least one image returned an exception/n{}", err);
            }
        }
    }
    
    return image_attachments;
}