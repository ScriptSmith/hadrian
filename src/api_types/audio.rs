#![allow(dead_code)]

//! OpenAI-compatible audio API types.
//!
//! Types for the audio endpoints:
//! - POST /v1/audio/speech - Text to speech
//! - POST /v1/audio/transcriptions - Speech to text
//! - POST /v1/audio/translations - Speech to English text

use serde::{Deserialize, Serialize};
use validator::Validate;

// ============================================================================
// Enums
// ============================================================================

/// Voice options for text-to-speech.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    Alloy,
    Ash,
    Ballad,
    Coral,
    Echo,
    Fable,
    Nova,
    Onyx,
    Sage,
    Shimmer,
    Verse,
    Marin,
    Cedar,
}

/// Audio output format for text-to-speech.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SpeechResponseFormat {
    #[default]
    Mp3,
    Opus,
    Aac,
    Flac,
    Wav,
    Pcm,
}

/// Stream format for text-to-speech streaming.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SpeechStreamFormat {
    /// Server-sent events format (not supported for tts-1 or tts-1-hd)
    Sse,
    /// Raw audio format
    #[default]
    Audio,
}

/// Response format for transcription/translation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AudioResponseFormat {
    #[default]
    Json,
    Text,
    Srt,
    VerboseJson,
    Vtt,
    /// Diarized JSON with speaker labels (gpt-4o-transcribe-diarize only)
    DiarizedJson,
}

/// Timestamp granularity options for transcription.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum TimestampGranularity {
    Word,
    Segment,
}

/// Additional information to include in transcription response.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionInclude {
    /// Include log probabilities in response
    Logprobs,
}

/// Chunking strategy for transcription.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionChunkingStrategy {
    Auto,
}

// ============================================================================
// Request Types
// ============================================================================

/// Create speech request (POST /v1/audio/speech)
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateSpeechRequest {
    /// One of the available TTS models: tts-1, tts-1-hd, or gpt-4o-mini-tts.
    pub model: String,

    /// The text to generate audio for. Maximum length is 4096 characters.
    #[validate(length(max = 4096))]
    pub input: String,

    /// The voice to use when generating the audio.
    pub voice: Voice,

    /// The format of the audio output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<SpeechResponseFormat>,

    /// The speed of the generated audio. Select a value from 0.25 to 4.0. Default is 1.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.25, max = 4.0))]
    pub speed: Option<f32>,

    /// Control the voice with additional instructions. Does not work with tts-1 or tts-1-hd.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(max = 4096))]
    pub instructions: Option<String>,

    /// The format to stream the audio in. Supported formats are sse and audio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_format: Option<SpeechStreamFormat>,
}

/// Create transcription request (POST /v1/audio/transcriptions)
///
/// Note: This endpoint accepts multipart/form-data. The `file` field
/// is a file upload, not a JSON field.
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateTranscriptionRequest {
    /// ID of the model to use. Options: gpt-4o-transcribe, gpt-4o-mini-transcribe,
    /// whisper-1, gpt-4o-transcribe-diarize.
    pub model: String,

    /// The language of the input audio in ISO-639-1 format (e.g., "en").
    /// Supplying the input language improves accuracy and latency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Optional text to guide the model's style or continue a previous audio segment.
    /// The prompt should match the audio language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    /// The format of the output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<AudioResponseFormat>,

    /// The sampling temperature, between 0 and 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub temperature: Option<f32>,

    /// The timestamp granularities to populate. Requires response_format=verbose_json.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_granularities: Option<Vec<TimestampGranularity>>,

    /// If true, stream the response using server-sent events.
    /// Not supported for whisper-1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Additional information to include in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<TranscriptionInclude>>,

    /// Chunking strategy for the transcription.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<TranscriptionChunkingStrategy>,

    /// Speaker names corresponding to known_speaker_references. Up to 4 speakers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub known_speaker_names: Option<Vec<String>>,

    /// Audio samples as data URLs for known speaker references. 2-10 seconds each.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub known_speaker_references: Option<Vec<String>>,
}

/// Create translation request (POST /v1/audio/translations)
///
/// Note: This endpoint accepts multipart/form-data. The `file` field
/// is a file upload, not a JSON field.
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateTranslationRequest {
    /// ID of the model to use. Only whisper-1 is currently available.
    pub model: String,

    /// Optional text to guide the model's style or continue a previous audio segment.
    /// The prompt should be in English.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    /// The format of the output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<AudioResponseFormat>,

    /// The sampling temperature, between 0 and 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub temperature: Option<f32>,
}

// ============================================================================
// Response Types
// ============================================================================

/// A word with timestamp information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionWord {
    /// The text content of the word.
    pub word: String,

    /// Start time of the word in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<f32>,

    /// End time of the word in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<f32>,
}

/// A segment of transcribed text with detailed information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionSegment {
    /// Unique identifier of the segment.
    pub id: i32,

    /// Seek offset of the segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seek: Option<i32>,

    /// Start time of the segment in seconds.
    pub start: f32,

    /// End time of the segment in seconds.
    pub end: f32,

    /// Text content of the segment.
    pub text: String,

    /// Array of token IDs for the text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<i32>>,

    /// Temperature parameter used for generating the segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Average logprob of the segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_logprob: Option<f32>,

    /// Compression ratio of the segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_ratio: Option<f32>,

    /// Probability of no speech in the segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_speech_prob: Option<f32>,
}

/// A diarized segment with speaker information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionDiarizedSegment {
    /// The type of segment.
    #[serde(rename = "type")]
    pub segment_type: String,

    /// Segment identifier.
    pub id: String,

    /// Start time of the segment in seconds.
    pub start: f32,

    /// End time of the segment in seconds.
    pub end: f32,

    /// Text content of the segment.
    pub text: String,

    /// The detected speaker label for this segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
}

/// Log probability information for a token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionLogprob {
    /// The token in the transcription.
    pub token: String,

    /// The log probability of the token.
    pub logprob: f32,

    /// The bytes of the token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i32>>,
}

/// Token usage details for transcription input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionInputTokenDetails {
    /// Number of text tokens in input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_tokens: Option<i64>,

    /// Number of audio tokens in input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<i64>,
}

/// Token-based usage statistics for transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionUsageTokens {
    /// The type of usage (always "tokens").
    #[serde(rename = "type")]
    pub usage_type: String,

    /// Total tokens used.
    pub total_tokens: i64,

    /// Input tokens used.
    pub input_tokens: i64,

    /// Output tokens used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i64>,

    /// Breakdown of input token types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_token_details: Option<TranscriptionInputTokenDetails>,
}

/// Duration-based usage statistics for transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionUsageDuration {
    /// The type of usage (always "duration").
    #[serde(rename = "type")]
    pub usage_type: String,

    /// Duration in seconds.
    pub seconds: i64,
}

/// Usage statistics for transcription (can be token-based or duration-based).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum TranscriptionUsage {
    Tokens(TranscriptionUsageTokens),
    Duration(TranscriptionUsageDuration),
}

/// Basic transcription response (JSON format).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionResponse {
    /// The transcribed text.
    pub text: String,

    /// Log probabilities of tokens (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<TranscriptionLogprob>>,

    /// Usage statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TranscriptionUsage>,
}

/// Verbose transcription response with timestamps and segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionVerboseResponse {
    /// The task type (always "transcribe").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,

    /// The language of the input audio.
    pub language: String,

    /// The duration of the input audio in seconds.
    pub duration: f32,

    /// The transcribed text.
    pub text: String,

    /// Extracted words and their timestamps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<TranscriptionWord>>,

    /// Segments of the transcribed text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<TranscriptionSegment>>,

    /// Usage statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TranscriptionUsage>,
}

/// Diarized transcription response with speaker labels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranscriptionDiarizedResponse {
    /// The task type (always "transcribe").
    pub task: String,

    /// The duration of the input audio in seconds.
    pub duration: f32,

    /// The concatenated transcript text.
    pub text: String,

    /// Segments with speaker annotations.
    pub segments: Vec<TranscriptionDiarizedSegment>,

    /// Usage statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TranscriptionUsage>,
}

/// Basic translation response (JSON format).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranslationResponse {
    /// The translated text.
    pub text: String,
}

/// Verbose translation response with timestamps and segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TranslationVerboseResponse {
    /// The language of the output translation (always "english").
    pub language: String,

    /// The duration of the input audio in seconds.
    pub duration: f32,

    /// The translated text.
    pub text: String,

    /// Segments of the translated text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<TranscriptionSegment>>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_speech_request_serialization() {
        let request = CreateSpeechRequest {
            model: "tts-1".to_string(),
            input: "Hello, world!".to_string(),
            voice: Voice::Alloy,
            response_format: Some(SpeechResponseFormat::Mp3),
            speed: Some(1.0),
            instructions: None,
            stream_format: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"tts-1\""));
        assert!(json.contains("\"input\":\"Hello, world!\""));
        assert!(json.contains("\"voice\":\"alloy\""));
        assert!(json.contains("\"response_format\":\"mp3\""));
        assert!(json.contains("\"speed\":1.0"));
    }

    #[test]
    fn test_create_speech_request_deserialization() {
        let json = r#"{
            "model": "tts-1-hd",
            "input": "The quick brown fox",
            "voice": "nova",
            "response_format": "opus",
            "speed": 1.5
        }"#;

        let request: CreateSpeechRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.model, "tts-1-hd");
        assert_eq!(request.input, "The quick brown fox");
        assert_eq!(request.voice, Voice::Nova);
        assert_eq!(request.response_format, Some(SpeechResponseFormat::Opus));
        assert_eq!(request.speed, Some(1.5));
    }

    #[test]
    fn test_create_transcription_request_serialization() {
        let request = CreateTranscriptionRequest {
            model: "whisper-1".to_string(),
            language: Some("en".to_string()),
            prompt: Some("Previous context".to_string()),
            response_format: Some(AudioResponseFormat::VerboseJson),
            temperature: Some(0.0),
            timestamp_granularities: Some(vec![
                TimestampGranularity::Word,
                TimestampGranularity::Segment,
            ]),
            stream: None,
            include: None,
            chunking_strategy: None,
            known_speaker_names: None,
            known_speaker_references: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"whisper-1\""));
        assert!(json.contains("\"language\":\"en\""));
        assert!(json.contains("\"response_format\":\"verbose_json\""));
        assert!(json.contains("\"word\""));
        assert!(json.contains("\"segment\""));
    }

    #[test]
    fn test_create_transcription_request_minimal() {
        let json = r#"{"model": "gpt-4o-transcribe"}"#;
        let request: CreateTranscriptionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.model, "gpt-4o-transcribe");
        assert!(request.language.is_none());
        assert!(request.prompt.is_none());
    }

    #[test]
    fn test_create_translation_request_serialization() {
        let request = CreateTranslationRequest {
            model: "whisper-1".to_string(),
            prompt: Some("Context".to_string()),
            response_format: Some(AudioResponseFormat::Text),
            temperature: Some(0.2),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"whisper-1\""));
        assert!(json.contains("\"response_format\":\"text\""));
    }

    #[test]
    fn test_transcription_response_serialization() {
        let response = TranscriptionResponse {
            text: "Hello, this is a test.".to_string(),
            logprobs: None,
            usage: Some(TranscriptionUsage::Tokens(TranscriptionUsageTokens {
                usage_type: "tokens".to_string(),
                total_tokens: 115,
                input_tokens: 14,
                output_tokens: Some(101),
                input_token_details: Some(TranscriptionInputTokenDetails {
                    text_tokens: Some(10),
                    audio_tokens: Some(4),
                }),
            })),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"text\":\"Hello, this is a test.\""));
        assert!(json.contains("\"total_tokens\":115"));
        assert!(json.contains("\"audio_tokens\":4"));
    }

    #[test]
    fn test_transcription_verbose_response() {
        let response = TranscriptionVerboseResponse {
            task: Some("transcribe".to_string()),
            language: "english".to_string(),
            duration: 8.47,
            text: "The beach was a popular spot.".to_string(),
            words: Some(vec![TranscriptionWord {
                word: "beach".to_string(),
                start: Some(0.5),
                end: Some(0.8),
            }]),
            segments: Some(vec![TranscriptionSegment {
                id: 0,
                seek: Some(0),
                start: 0.0,
                end: 3.32,
                text: "The beach was a popular spot.".to_string(),
                tokens: Some(vec![50364, 440, 7534]),
                temperature: Some(0.0),
                avg_logprob: Some(-0.286),
                compression_ratio: Some(1.2),
                no_speech_prob: Some(0.01),
            }]),
            usage: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"language\":\"english\""));
        assert!(json.contains("\"duration\":8.47"));
        assert!(json.contains("\"word\":\"beach\""));
    }

    #[test]
    fn test_transcription_diarized_response() {
        let response = TranscriptionDiarizedResponse {
            task: "transcribe".to_string(),
            duration: 42.7,
            text: "Agent: Thanks for calling.".to_string(),
            segments: vec![TranscriptionDiarizedSegment {
                segment_type: "transcript.text.segment".to_string(),
                id: "seg_001".to_string(),
                start: 0.0,
                end: 5.2,
                text: "Thanks for calling.".to_string(),
                speaker: Some("agent".to_string()),
            }],
            usage: Some(TranscriptionUsage::Duration(TranscriptionUsageDuration {
                usage_type: "duration".to_string(),
                seconds: 43,
            })),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"task\":\"transcribe\""));
        assert!(json.contains("\"speaker\":\"agent\""));
        assert!(json.contains("\"seconds\":43"));
    }

    #[test]
    fn test_translation_response() {
        let response = TranslationResponse {
            text: "Hello, how are you?".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"text\":\"Hello, how are you?\""));
    }

    #[test]
    fn test_voice_enum() {
        assert_eq!(serde_json::to_string(&Voice::Alloy).unwrap(), "\"alloy\"");
        assert_eq!(serde_json::to_string(&Voice::Nova).unwrap(), "\"nova\"");
        assert_eq!(
            serde_json::to_string(&Voice::Shimmer).unwrap(),
            "\"shimmer\""
        );
    }

    #[test]
    fn test_speech_response_format_enum() {
        assert_eq!(
            serde_json::to_string(&SpeechResponseFormat::Mp3).unwrap(),
            "\"mp3\""
        );
        assert_eq!(
            serde_json::to_string(&SpeechResponseFormat::Opus).unwrap(),
            "\"opus\""
        );
        assert_eq!(
            serde_json::to_string(&SpeechResponseFormat::Pcm).unwrap(),
            "\"pcm\""
        );
    }

    #[test]
    fn test_audio_response_format_enum() {
        assert_eq!(
            serde_json::to_string(&AudioResponseFormat::Json).unwrap(),
            "\"json\""
        );
        assert_eq!(
            serde_json::to_string(&AudioResponseFormat::VerboseJson).unwrap(),
            "\"verbose_json\""
        );
        assert_eq!(
            serde_json::to_string(&AudioResponseFormat::DiarizedJson).unwrap(),
            "\"diarized_json\""
        );
    }

    #[test]
    fn test_timestamp_granularity_enum() {
        assert_eq!(
            serde_json::to_string(&TimestampGranularity::Word).unwrap(),
            "\"word\""
        );
        assert_eq!(
            serde_json::to_string(&TimestampGranularity::Segment).unwrap(),
            "\"segment\""
        );
    }

    #[test]
    fn test_usage_deserialization_tokens() {
        let json = r#"{
            "type": "tokens",
            "total_tokens": 115,
            "input_tokens": 14,
            "output_tokens": 101
        }"#;

        let usage: TranscriptionUsage = serde_json::from_str(json).unwrap();
        match usage {
            TranscriptionUsage::Tokens(t) => {
                assert_eq!(t.total_tokens, 115);
                assert_eq!(t.input_tokens, 14);
            }
            _ => panic!("Expected Tokens variant"),
        }
    }

    #[test]
    fn test_usage_deserialization_duration() {
        let json = r#"{
            "type": "duration",
            "seconds": 43
        }"#;

        let usage: TranscriptionUsage = serde_json::from_str(json).unwrap();
        match usage {
            TranscriptionUsage::Duration(d) => {
                assert_eq!(d.seconds, 43);
            }
            _ => panic!("Expected Duration variant"),
        }
    }
}
