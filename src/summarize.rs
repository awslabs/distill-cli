use aws_config::SdkConfig;
use aws_sdk_bedrockruntime::{primitives::Blob, Client};

use anyhow::{anyhow, Error};

use config::{Config, File};
use serde_json::json;
use spinoff::Spinner;
use std::str::from_utf8;

pub async fn summarize_text(
    config: &SdkConfig,
    transcribed_text: &str,
    spinner: &mut Spinner,
) -> Result<String, Error> {
    let client = Client::new(config);
    let settings = Config::builder()
        .add_source(File::with_name("config.toml"))
        .build()?;

    let prompt_template = settings.get_string("prompt.template").unwrap_or_default();

    let prompt = format!("{prompt_template}\n\n{transcribed_text}");

    // We're using the Anthropic Claude Messages API by default.
    // If you switch models, you may need to update `messages`
    // and/or `body`.
    // https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters.html
    // Claude: https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters-anthropic-claude-messages.html
    let messages = json!([
        {
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": prompt,
                }
            ]
        }
    ]);

    let body = json!(
        {
            "anthropic_version": settings.get_string("anthropic.anthropic_version").unwrap_or_default(),
            "max_tokens": settings.get_int("model.max_tokens").unwrap_or_default(),
            "system": settings.get_string("anthropic.system").unwrap_or_default(),
            "messages": messages,
            "temperature": settings.get_int("model.temperature").unwrap_or_default(),
            "top_p": settings.get_int("model.top_p").unwrap_or_default(),
            "top_k": settings.get_int("model.top_k").unwrap_or_default(),
        }
    )
    .to_string();

    let blob_body = Blob::new(body);

    spinner.update_text("Summarizing transcription...");
    let response = client
        .invoke_model()
        .body(blob_body)
        .content_type("application/json")
        .accept("application/json")
        .model_id(settings.get_string("model.model_id").unwrap_or_default())
        .send()
        .await;

    match response {
        Ok(output) => {
            let response_body = from_utf8(output.body.as_ref()).unwrap_or("");
            let response_json: serde_json::Value = serde_json::from_str(response_body).unwrap();

            let summarization = response_json["content"][0]["text"]
                .as_str()
                .unwrap()
                .replace("\\n", "\n");
            Ok(summarization.to_string())
        }
        Err(e) => Err(anyhow!(e)),
    }
}
