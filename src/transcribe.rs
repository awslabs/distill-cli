use aws_config::SdkConfig;
use aws_sdk_transcribe::types::{
    LanguageCode, Media, MediaFormat, Settings, TranscriptionJobStatus,
};
use aws_sdk_transcribe::Client;

use anyhow::{anyhow, bail, Context, Error};
use infer::get_from_path;
use serde_json::Value;
use spinoff::{spinners, Spinner};
use std::path::Path;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

pub async fn transcribe_audio(
    config: &SdkConfig,
    file_path: &Path,
    s3_uri: &str,
    spinner: &mut Spinner,
) -> Result<String, Error> {
    let client = Client::new(config);

    spinner.update(spinners::Dots7, "Submitting transcription job", None);
    let job_name = format!("transcription-{}", Uuid::new_v4()); // Generate a unique job name
    let media = Media::builder().media_file_uri(s3_uri).build();

    let media_format = match get_from_path(file_path) {
        Ok(Some(kind)) => match kind.mime_type() {
            "audio/amr" => MediaFormat::Amr,
            "audio/flac" => MediaFormat::Flac,
            "audio/m4a" => MediaFormat::M4A,
            "audio/mpeg" => MediaFormat::Mp3,
            "audio/mp4" => MediaFormat::Mp4,
            "video/mp4" => MediaFormat::Mp4,
            "audio/ogg" => MediaFormat::Ogg,
            "audio/wav" => MediaFormat::Wav,
            "audio/webm" => MediaFormat::Webm,
            _ => {
                bail!("\nUnsupported media format: {}", kind.mime_type());
            }
        },
        Ok(None) => {
            bail!("\nUnable to determine media format from file extension");
        }
        Err(err) => {
            bail!("\nError determining media format: {}", err);
        }
    };

    let settings = Settings::builder()
        .show_speaker_labels(true)
        .max_speaker_labels(10)
        .channel_identification(false)
        .build();

    let _job = client
        .start_transcription_job()
        .transcription_job_name(&job_name)
        .language_code(LanguageCode::EnUs)
        .media_format(media_format)
        .media(media)
        .settings(settings)
        .send()
        .await?;

    println!();
    spinner.update(
        spinners::Dots7,
        "Waiting for transcription to complete...",
        None,
    );
    let mut poll_interval = Duration::from_secs(5);
    let mut job_details = client
        .get_transcription_job()
        .transcription_job_name(&job_name)
        .send()
        .await?;

    while let Some(status) = job_details
        .transcription_job
        .as_ref()
        .and_then(|j| j.transcription_job_status.as_ref())
    {
        match status {
            TranscriptionJobStatus::InProgress => {
                sleep(poll_interval).await;
                job_details = client
                    .get_transcription_job()
                    .transcription_job_name(&job_name)
                    .send()
                    .await?;
                println!();
                poll_interval *= 2; // Exponential backoff to show progress
            }
            TranscriptionJobStatus::Completed => {
                break;
            }
            _ => {
                // ToDo Handle other states, e.g., Failed
                break;
            }
        }
    }

    match job_details
        .transcription_job
        .as_ref()
        .and_then(|j| j.transcription_job_status.as_ref())
    {
        Some(TranscriptionJobStatus::Completed) => {
            if let Some(transcript_uri) = job_details
                .transcription_job
                .and_then(|j| j.transcript)
                .and_then(|t| t.transcript_file_uri)
            {
                spinner.update(spinners::Dots7, "Transcription job complete", None);
                let res = reqwest::get(transcript_uri).await?;
                let body = res.text().await?;
                let final_transcript = convert_transcribe_json(&body)?;
                Ok(final_transcript)
            } else {
                println!("Transcript file URI is missing.");
                Ok("Transcript file URI is missing.".to_string())
            }
        }
        Some(TranscriptionJobStatus::Failed) => {
            if let Some(reason) = job_details.transcription_job.and_then(|j| j.failure_reason) {
                println!("Transcription job failed: {}", reason);
            } else {
                println!("Transcription job failed for an unknown reason.");
            }
            Ok("Transcription job failed.".to_string())
        }
        _ => Ok(
            "Job ended with an unexpected status or status could not be determined.".to_string(),
        ),
    }
}

pub fn convert_transcribe_json(json_string: &str) -> Result<String, Error> {
    let v: Value = serde_json::from_str(json_string).with_context(|| "Failed to parse JSON")?;

    let mut final_transcript = String::new();
    let mut current_speaker: Option<String> = None;
    let mut current_text = String::new();

    for item in v["results"]["items"].as_array().unwrap() {
        match item["type"].as_str().unwrap() {
            "pronunciation" => {
                let content = item["alternatives"][0]["content"]
                    .as_str()
                    .ok_or_else(|| anyhow!("Missing pronunciation content data"))?;
                let speaker_label = item["speaker_label"]
                    .as_str()
                    .ok_or_else(|| anyhow!("Missing 'speaker_label' data"))?;

                if let Some(current_speaker_label) = current_speaker.as_ref() {
                    if current_speaker_label != speaker_label {
                        if !current_text.is_empty() {
                            final_transcript.push_str(&format!(
                                "{}: {}\n",
                                current_speaker_label,
                                current_text.trim()
                            ));
                        }
                        current_speaker = Some(speaker_label.to_string());
                        current_text = content.to_string();
                    } else {
                        current_text.push(' ');
                        current_text.push_str(content);
                    }
                } else {
                    current_speaker = Some(speaker_label.to_string());
                    current_text = content.to_string();
                }
            }
            "punctuation" => {
                let content = item["alternatives"][0]["content"]
                    .as_str()
                    .ok_or_else(|| anyhow!("Missing punctuation content data"))?;
                current_text.push_str(content);
            }
            _ => {}
        }
    }

    if let Some(speaker_label) = current_speaker {
        if !current_text.is_empty() {
            final_transcript.push_str(&format!("{}: {}\n", speaker_label, current_text.trim()));
        }
    }

    Ok(final_transcript)
}
