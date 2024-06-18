# Summary

The Distill CLI uses Amazon Transcribe and Amazon Bedrock to create summaries of your audio recordings (e.g., meetings, podcasts, etc.) directly from the command line. It is based on the open source tool: [Amazon Bedrock Audio Summarizer](https://github.com/aws-samples/amazon-bedrock-audio-summarizer).

# Supported audio formats

Like the [Amazon Bedrock Audio Summarizer](https://github.com/aws-samples/amazon-bedrock-audio-summarizer), the Distill CLI takes a dependency on Amazon Transcribe, and as such, supports the following [media formats](https://docs.aws.amazon.com/transcribe/latest/dg/how-input.html#how-input-audio): AMR, FLAC, M4A, MP3, MP4, Ogg, WebM, WAV.

# A note on regions

By default, the Distill CLI inherits credentials and configuration details from the AWS CLI. Since Bedrock is not yet available in every region, ensure that the default region in your AWS config is on the list of [supported Bedrock regions](https://docs.aws.amazon.com/bedrock/latest/userguide/bedrock-regions.html). 

**Note**: If no region is set in your AWS CLI config, the Distill CLI will default to `us-east-1`.

To check your defaults, run:

```bash
aws configure list
```

# Install the Distill CLI

This project is written in Rust, and uses the AWS SDK for Rust to manage credentials and access AWS services, including S3, Transcribe and Bedrock. 

**IMPORTANT**: By using the Distill CLI, you may incur charges to your AWS account. 

## Prerequisites 

Before using the Distill CLI, you'll need: 

- [An AWS Account](https://portal.aws.amazon.com/gp/aws/developer/registration/index.html) configured with an [IAM user that has permissions](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html#Using_CreateAccessKey) to Amazon Transcribe, Amazon Bedrock, and Amazon S3. 
- [Configure the AWS CLI](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html) to access your AWS account.
- An S3 bucket to store audio files, or [create a new one](https://docs.aws.amazon.com/AmazonS3/latest/userguide/creating-bucket.html). 
- [Access to Anthropic's Claude 3](https://console.aws.amazon.com/bedrock/home?#/models) via the AWS Bedrock Console.
- [Rust and Cargo](https://www.rust-lang.org/tools/install) installed.

## Step 1: Clone the repo 

```bash
git clone https://github.com/awslabs/distill-cli.git && cd distill-cli
```

## Step 2: Build from source

Run the following command to build the Distill CLI from source. This will compile the code and create an optimized binary in `target/release`.

```bash
$ cargo build --release
```

You should see a message like this when the build is complete:

```bash
Compiling distill-cli v0.1.0 (/Projects/distill-cli)
    Finished release [optimized] target(s) in 18.07s
```

# Usage

Once installed, it's easy to use the Distill CLI. Each operation starts with:

```bash
./target/release/distill-cli [arguments]
```

Here's a simple example. By default, the Distill CLI will print the summary to terminal unless otherwise specified:

```bash
./target/release/distill-cli -i meeting.m4a
```

You'll see something similar to the following:

```bash
$ ./target/release/distill-cli -i meeting.m4a

🧙 Welcome to Distill CLI
✔ Choose a destination S3 bucket for your audio file · mys3bucket
⠐ Uploading file to S3...
⠐ Using bucket region eu-west-2...
⠒ Submitting transcription job
⠤ Waiting for transcription to complete...
⠤ Waiting for transcription to complete...
✓ Done!

Summary:
Here is a summary of the conversation:

The speakers discussed the recent Premier League matches involving Arsenal, Manchester City, and Liverpool. Arsenal beat Luton Town in their match, while Manchester City also won their game 4-1. This leaves Arsenal tied on points with Manchester City, but with a better goal differential, putting them temporarily in first place ahead of City. However, the speakers expect Liverpool, who are currently one or two points behind Arsenal, to regain the lead after their upcoming match against an opponent perceived as weak.

Key action items and follow-ups:

- Monitor Liverpool's next match results, as they are expected to go back into first place in the Premier League standings
- Keep track of the evolving points totals and goal differentials for Arsenal, Manchester City, and Liverpool as the title race continues
...
```

# Options 

As this is a simple CLI, there are only a few options.

| Option | Required | Description |
| - | - | - |
| `-i`, `--input-audio-file` | Yes | Specify the audio file to be summarized. | 
| `-o`, `--output-type` | No | Specify the output format of the summary. Default is terminal.<br> **Accepted values**: `terminal`, `text`, `word`, `markdown`, `slack`  | 
| `-h`, `--help` | No | Provides help for the Distill CLI. |

# Config settings

`config.toml` is used to manage config settings for the Distill CLI and must be in the execution directory of `distill-cli`.  

## How to adjust model values

The CLI is intended as a proof-of-concept, and as such is designed to support Anthropic's Claude 3 foundation model. The model, along with values such as max tokens and temperature are specified in [`config.toml`](./config.toml).

```
[model]
model_id = "anthropic.claude-3-sonnet-20240229-v1:0"
max_tokens = 2000
temperature = 1.0
top_p = 0.999
top_k = 40
```

**IMPORTANT**: If changing to a model not provided by Anthropic, code changes may be required to `messages` and `body` in [`summarizer.rs`](./src/summarize.rs), as the structure of the messages passed to Bedrock may change. Anthropic's models, for example, currently use the [Messages API](https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters-anthropic-claude-messages.html). 

## Supported Bedrock models

You can view a list of available models at [Amazon Bedrock base model IDs](https://docs.aws.amazon.com/bedrock/latest/userguide/model-ids.html), or via the command line:

```
$ aws bedrock list-foundation-models

{
    "modelSummaries": [
        {
            "modelArn": "arn:aws:bedrock:us-east-1::foundation-model/amazon.titan-tg1-large",
            "modelId": "amazon.titan-tg1-large",
            "modelName": "Titan Text Large",
            "providerName": "Amazon",
            "inputModalities": [
                "TEXT"
            ],
            "outputModalities": [
                "TEXT"
            ],
            "responseStreamingSupported": true,
            "customizationsSupported": [],
            "inferenceTypesSupported": [
                "ON_DEMAND"
            ]
        },
        {
            "modelArn": "arn:aws:bedrock:us-east-1::foundation-model/amazon.titan-image-generator-v1:0",
            "modelId": "amazon.titan-image-generator-v1:0",
            "modelName": "Titan Image Generator G1",
            "providerName": "Amazon",
            "inputModalities": [
                "TEXT",
                "IMAGE"
            ],
            "outputModalities": [
                "IMAGE"
            ],
            "customizationsSupported": [
                "FINE_TUNING"
            ],
            "inferenceTypesSupported": [
                "PROVISIONED"
            ]
        },
        ...
    ]
}
```

## Additional output settings

### Slack

To output a summary to a Slack channel, create a [Slack webhook](https://api.slack.com/messaging/webhooks), and update and uncomment the enpdoint in `config.toml`. If you do not set the endpoint, or if the endpoint is commented out, you'll receive the error "Slack webhook endpoint is not configured. Skipping Slack notification.".

```
...
# =============================================================================
# Slack Integration
# =============================================================================

[slack]
# webhook_endpoint = "https://hooks.slack.com/workflows/XYZ/ABC/123"
```
