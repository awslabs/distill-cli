mod summarize;
mod transcribe;

use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_config::{Region, SdkConfig};
use clap::Parser;
use config::{Config, File as ConfigFile};
use docx_rs::{Docx, Paragraph, Run};
use spinoff::{spinners, Color, Spinner};

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use dialoguer::{theme::ColorfulTheme, Select};

#[derive(Debug, Parser)]
#[clap(
    about = "Distill CLI can summarize an audio file (e.g., a meeting) using Amazon Transcribe and Amazon Bedrock."
)]
struct Opt {
    #[clap(short, long)]
    input_audio_file: String,

    #[clap(
        short,
        long,
        value_enum,
        default_value = "Terminal",
        ignore_case = true
    )]
    output_type: OutputType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum OutputType {
    Terminal,
    Text,
    Word,
    Markdown,
}

#[::tokio::main]
async fn main() -> Result<()> {
    let config = load_config(None).await;

    let settings = Config::builder()
        .add_source(ConfigFile::with_name("./config.toml"))
        .build()?;

    let s3_bucket_name = settings
        .get_string("aws.s3_bucket_name")
        .unwrap_or_default();

    let Opt {
        input_audio_file,
        output_type,
    } = Opt::parse();

    let s3_client = Client::new(&config);

    let mut bucket_name = String::new();

    println!("🧙 Welcome to Distill CLI");

    let resp = &list_buckets(&s3_client).await;

    if !s3_bucket_name.is_empty() {
        if resp
            .as_ref()
            .ok()
            .and_then(|buckets| buckets.iter().find(|b| b.as_str() == s3_bucket_name))
            .is_some()
        {
            println!("📦 S3 bucket name: {}", s3_bucket_name);
            bucket_name = s3_bucket_name;
        } else {
            println!(
                "Error: The configured S3 bucket '{}' was not found.",
                s3_bucket_name
            );
        }
    }

    if bucket_name.is_empty() {
        match resp {
            Ok(bucket_names) => {
                let selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Choose a destination S3 bucket for your audio file")
                    .default(0)
                    .items(&bucket_names[..])
                    .interact()?;

                bucket_name.clone_from(&bucket_names[selection]);
            }
            Err(err) => {
                println!("Error getting bucket list: {}", err);
                bail!("\nError getting bucket list: {}", err);
            }
        };
    }

    if bucket_name.is_empty() {
        bail!("\nNo valid S3 bucket found. Please check your AWS configuration.");
    }

    let mut spinner = Spinner::new(spinners::Dots7, "Uploading file to S3...", Color::Green);

    // Load the bucket region and create a new client to use that region
    let region = bucket_region(&s3_client, &bucket_name).await?;
    println!();
    spinner.update(
        spinners::Dots7,
        format!("Using bucket region {}", region),
        None,
    );
    let regional_config = load_config(Some(region)).await;
    let regional_s3_client = Client::new(&regional_config);

    // Handle conversion of relative paths to absolute paths
    let file_path = Path::new(&input_audio_file);
    let file_name = file_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let absolute_path = shellexpand::tilde(file_path.to_str().unwrap()).to_string();
    let absolute_path = Path::new(&absolute_path);

    if !absolute_path.exists() {
        bail!("\nThe path {} does not exist.", absolute_path.display());
    }

    let canonicalized_path = absolute_path.canonicalize()?;
    let body = ByteStream::from_path(&canonicalized_path)
        .await
        .with_context(|| format!("Error loading file: {}", canonicalized_path.display()))?;

    let _upload_result = regional_s3_client
        .put_object()
        .bucket(&bucket_name)
        .key(&file_name)
        .body(body)
        .send()
        .await
        .context("Failed to upload to S3")?;

    let s3_uri = format!("s3://{}/{}", bucket_name, file_name);

    println!();
    spinner.update(spinners::Dots7, "Summarizing text...", None);

    // Transcribe the audio
    let transcription =
        transcribe::transcribe_audio(&regional_config, file_path, &s3_uri, &mut spinner).await?;

    // Summarize the transcription
    spinner.update(spinners::Dots7, "Summarizing text...", None);
    let summarized_text = summarize::summarize_text(&config, &transcription, &mut spinner).await?;

    match output_type {
        OutputType::Word => {
            let output_file_path_word = Path::new("summary.docx");
            let file = File::create(output_file_path_word)
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

            // Creating a new document and adding paragraphs
            let doc = Docx::new()
                .add_paragraph(Paragraph::new().add_run(Run::new().add_text(&summarized_text)))
                .add_paragraph(Paragraph::new().add_run(Run::new().add_text("\n\n")))
                .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Transcription:\n")))
                .add_paragraph(Paragraph::new().add_run(Run::new().add_text(&transcription)));

            // Building and saving the document
            doc.build()
                .pack(file)
                .map_err(|e| anyhow::anyhow!("Error writing Word document: {}", e))?;

            spinner.success("Done!");
            println!(
                "💾 Summary and transcription written to {}",
                output_file_path_word.display()
            );
        }
        OutputType::Text => {
            let output_file_path_txt = Path::new("summary.txt");
            let mut file = File::create(output_file_path_txt)
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

            file.write_all(summarized_text.as_bytes())
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;
            file.write_all(b"\n\nTranscription:\n")
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;
            file.write_all(transcription.as_bytes())
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

            spinner.success("Done!");
            println!(
                "💾 Summary and transcription written to {}",
                output_file_path_txt.display()
            );
        }
        OutputType::Terminal => {
            spinner.success("Done!");
            println!();
            println!("Summary:\n{}\n", summarized_text);
            println!("Transcription:\n{}\n", transcription);
        }
        OutputType::Markdown => {
            let output_file_path_md = Path::new("summary.md");
            let mut file = File::create(output_file_path_md)
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

            let summary_md = format!("# Summary\n\n{}", summarized_text);
            let mut transcription_md = format!("\n\n# Transcription\n\n{}", transcription);
            transcription_md = transcription_md.replace("spk_", "\nspk_");
            let markdown_content = format!("{}{}", summary_md, transcription_md);

            file.write_all(markdown_content.as_bytes())
                .map_err(|e| anyhow::anyhow!("Error writing Markdown file: {}", e))?;

            spinner.success("Done!");
            println!(
                "💾 Summary and transcription written to {}",
                output_file_path_md.display()
            );
        }
    }

    Ok(())
}

// Load the user's aws config, default region to us-east-1 if none is provided or can be found
async fn load_config(region: Option<Region>) -> SdkConfig {
    let mut config = aws_config::from_env();
    match region {
        Some(region) => config = config.region(region),
        None => {
            config = config.region(RegionProviderChain::default_provider().or_else("us-east-1"))
        }
    }
    config.load().await
}

async fn list_buckets(client: &Client) -> Result<Vec<String>> {
    let resp = client.list_buckets().send().await?;
    let buckets = resp.buckets();

    let bucket_names: Vec<String> = buckets
        .iter()
        .map(|bucket| bucket.name().unwrap_or_default().to_string())
        .collect();

    Ok(bucket_names)
}

async fn bucket_region(client: &Client, bucket_name: &str) -> Result<Region> {
    let resp = client
        .get_bucket_location()
        .bucket(bucket_name)
        .send()
        .await?;

    let location_constraint = resp
        .location_constraint()
        .context("Bucket has no location_constraint")?;

    if location_constraint.as_str() == "" {
        Ok(Region::new("us-east-1"))
    } else {
        Ok(Region::new(location_constraint.as_str().to_owned()))
    }
}
