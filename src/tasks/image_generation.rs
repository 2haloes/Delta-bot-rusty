use std::{env, time::Duration, u32};

use poise::serenity_prelude as serenity;
use async_openai::{types::{CreateImageRequestArgs, Image, ImageModel, ImageSize, ResponseFormat}, Client};
use base64::prelude::*;
use reqwest::{header::HeaderValue, Response};
use ::serenity::all::CreateSelectMenuOption;
use serenity::all::{CreateAttachment, Message};
use tokio::time::sleep;

use crate::{Error, FunctionData, JsonObject};

use super::handle_errors::return_error;

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

// #[derive(Debug, poise::Modal)]
// #[name = "Runpod Generation"]
// struct ServerlessModal {
//     #[name = "Prompt"]
//     prompt: String,
//     #[name = "Width Ratio (Default: 1)"]
//     width_ratio: Option<String>,
//     #[name = "Height Ratio (Default: 1)"]
//     height_ratio: Option<String>,
//     #[name = "Number of generations (Default: 1)"]
//     num_gen: Option<String>
// }

// #[derive(Debug, poise::Modal)]
// #[name = "DALL-E Generation"]
// struct DalleModal {
//     #[name = "Prompt"]
//     prompt: String,
//     #[name = "Number of generations (Default: 1)"]
//     num_gen: Option<String>
// }

// #[poise::command(prefix_command, slash_command)]
// pub async fn imagegen(ctx: crate::Context<'_>) -> Result<(), Error> {

//     let bug_message = Message::default();

//     // let function_json_string = match tokio::fs::read_to_string("assets/functions.json").await
//     //     {
//     //         Ok(t) => t,
//     //         Err(e) => return_error(bug_message.clone(), e.to_string()).await.unwrap(),
//     //     };
//     // let function_object: JsonObject = match serde_json::from_str(&function_json_string)
//     //     {
//     //         Ok(t) => t,
//     //         Err(e) => return_error(bug_message.clone(), e.to_string()).await.unwrap(),
//     //     };
//     // let current_function: FunctionData = match function_object.function_data.into_iter()
//     //     .filter(|function| function.function_command == command_string).next()
//     //     {
//     //         Some(t) => t,
//     //         None => return_error(bug_message.clone(), "Unable to process current function string".to_owned()).await.unwrap(),
//     //     };

//     // let model_options: Vec<CreateSelectMenuOption> = function_object.function_data.into_iter().map(|command_index: u32, command_object: FunctionData| 
//     //     CreateSelectMenuOption::new(command_object.function_friendly_name, command_index)
//     // ) ;

//     // let mut model_options: Vec<CreateSelectMenuOption> = Vec::new();
//     // let mut current_index: usize = 0;
    
//     // for (command_object) in function_object.function_data.into_iter() {
//     //     model_options.push(CreateSelectMenuOption::new(command_object.function_friendly_name, current_index.to_string()));
//     //     current_index += 1;
//     // };

//     let model_options: Vec<CreateSelectMenuOption> = vec![
//         CreateSelectMenuOption::new("Explosion", "0".to_string()),
//         CreateSelectMenuOption::new("Knife", "1".to_string())
//     ];
//     let reply = {
//         let components: Vec<serenity::CreateActionRow> = vec![poise::serenity_prelude::CreateActionRow::SelectMenu(serenity::CreateSelectMenu::new("model_select",
//             serenity::builder::CreateSelectMenuKind::String { options: model_options }
//     ))];

//         poise::CreateReply::default()
//             .content("Please select which model to use")
//             .components(components)
//     };

//     let sent_message = ctx.send(reply).await?;

//     while let Some(mci) = serenity::ComponentInteractionCollector::new(ctx.serenity_context())
//         .timeout(std::time::Duration::from_secs(120))
//         .filter(move |mci| mci.data.custom_id == "model_select")
//         .await
//     {
//         let _test = mci.clone();
//         let full_message = sent_message.clone().into_message().await?;
//         let _message_deleted = full_message.delete(ctx).await?;
//         let data =
//             poise::execute_modal_on_component_interaction::<ServerlessModal>(ctx, mci.clone(), None, None).await?;
//         let data_unwrapped = data.unwrap();
//         let width_ratio: f32 = data_unwrapped.width_ratio.unwrap_or("1".to_string()).parse().unwrap();
//         let height_ratio: f32 = data_unwrapped.height_ratio.unwrap_or("1".to_string()).parse().unwrap();
//         // This is a fixed value from 1024*1024 (this being the default SDXL height and width)
//         let total_pixel_count: f32 = 1048576.0;
//         // Calculate the image size based on the aspect ratio and total number of pixels the model allows
//         // For example, Stable Diffusion XL supports 1024x1024 so the total pixesl would be the result of 1024*1024!
//         let height = ((total_pixel_count * (height_ratio / width_ratio)).sqrt()).round();
//         let width = ((width_ratio / height_ratio) * height).round();
    
    
//         ctx.say(format!("Hello, the width is {} and the height is {}! Thank you for asking <@{}>", width, height, mci.user.id)).await?;
//     }
//     Ok(())
// }

/*
    This generates images using DALL-E
    It uses the openai-async library for making calls
*/
pub async fn generate_dalle (prompt_text: String, msg: Message) -> Vec<CreateAttachment> {
    let client = Client::new();
    let request = match CreateImageRequestArgs::default()
        .prompt(prompt_text)
        .n(1)
        .response_format(ResponseFormat::B64Json)
        .size(ImageSize::S1024x1024)
        .user("Delta-Bot")
        .model(ImageModel::DallE3)
        .build()
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };

    let response = match client.images().create(request).await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
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
                return_error(msg.clone(), "Expected Base64 from DALL-E, got a different output instead".to_owned()).await.unwrap()
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
pub async fn generate_runpod_image (prompt_text: String, model_ref: String, msg: Message) -> Vec<CreateAttachment> {
    let runpod_auth_key = match env::var("RUNPOD_API_KEY")
        {
            Ok(t) => t,
            Err(_) => return_error(msg.clone(), "No runpod API key found".to_owned()).await.unwrap(),
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
                \"height\": 1024,
                \"width\": 1024,
                \"scheduler\": \"K_EULER\",
                \"num_inference_steps\": 40
            }}
        }}", prompt_text))
        .send()
        .await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };
       

    let run_response_json: RunResponseObject = match run_response.json()
        .await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
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
                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
            };
        

        status_response_json = match status_response.json()
            .await
            {
                Ok(t) => t,
                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
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
        None => return_error(msg.clone(), "No generated image data found".to_owned()).await.unwrap(),
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