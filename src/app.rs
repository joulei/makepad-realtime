use base64::{Engine as _, engine::general_purpose};
use makepad_widgets::*;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// OpenAI Realtime API Demo Implementation
//
// This demonstrates a basic integration with OpenAI's Realtime API for conversational audio.
// Key components:
// - WebSocket connection to OpenAI with proper authentication
// - Real-time audio streaming (24kHz PCM16) with format conversion
// - Server-side VAD (Voice Activity Detection) for turn management
// - Audio interruption handling to prevent feedback loops
// - Full duplex audio: simultaneous recording and playback

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum OpenAIRealtimeMessage {
    #[serde(rename = "session.update")]
    SessionUpdate { session: SessionConfig },
    #[serde(rename = "input_audio_buffer.append")]
    InputAudioBufferAppend {
        audio: String, // base64 encoded audio
    },
    #[serde(rename = "input_audio_buffer.commit")]
    InputAudioBufferCommit,
    #[serde(rename = "response.create")]
    ResponseCreate { response: ResponseConfig },
    #[serde(rename = "conversation.item.create")]
    ConversationItemCreate { item: ConversationItem },
    #[serde(rename = "conversation.item.truncate")]
    ConversationItemTruncate {
        item_id: String,
        content_index: u32,
        audio_end_ms: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        event_id: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SessionConfig {
    pub modalities: Vec<String>,
    pub instructions: String,
    pub voice: String,
    pub input_audio_format: String,
    pub output_audio_format: String,
    pub input_audio_transcription: Option<TranscriptionConfig>,
    pub turn_detection: Option<TurnDetectionConfig>,
    pub tools: Vec<serde_json::Value>,
    pub tool_choice: String,
    pub temperature: f32,
    pub max_response_output_tokens: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TranscriptionConfig {
    pub model: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TurnDetectionConfig {
    #[serde(rename = "type")]
    pub detection_type: String,
    pub threshold: f32,
    pub prefix_padding_ms: u32,
    pub silence_duration_ms: u32,
    pub interrupt_response: bool,
    pub create_response: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResponseConfig {
    pub modalities: Vec<String>,
    pub instructions: Option<String>,
    pub voice: Option<String>,
    pub output_audio_format: Option<String>,
    pub tools: Option<Vec<serde_json::Value>>,
    pub tool_choice: Option<String>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConversationItem {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub item_type: String,
    pub status: Option<String>,
    pub role: String,
    pub content: Vec<ContentPart>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "input_text")]
    InputText { text: String },
    #[serde(rename = "input_audio")]
    InputAudio {
        audio: String,
        transcript: Option<String>,
    },
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "audio")]
    Audio {
        audio: String,
        transcript: Option<String>,
    },
}

// Incoming message types from OpenAI
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum OpenAIRealtimeResponse {
    #[serde(rename = "error")]
    Error { error: ErrorDetails },
    #[serde(rename = "session.created")]
    SessionCreated { session: serde_json::Value },
    #[serde(rename = "session.updated")]
    SessionUpdated { session: serde_json::Value },
    #[serde(rename = "conversation.item.created")]
    ConversationItemCreated { item: serde_json::Value },
    #[serde(rename = "conversation.item.truncated")]
    ConversationItemTruncated { item: serde_json::Value },
    #[serde(rename = "response.audio.delta")]
    ResponseAudioDelta {
        response_id: String,
        item_id: String,
        output_index: u32,
        content_index: u32,
        delta: String, // base64 encoded audio
    },
    #[serde(rename = "response.audio.done")]
    ResponseAudioDone {
        response_id: String,
        item_id: String,
        output_index: u32,
        content_index: u32,
    },
    #[serde(rename = "response.text.delta")]
    ResponseTextDelta {
        response_id: String,
        item_id: String,
        output_index: u32,
        content_index: u32,
        delta: String,
    },
    #[serde(rename = "response.audio_transcript.delta")]
    ResponseAudioTranscriptDelta {
        response_id: String,
        item_id: String,
        output_index: u32,
        content_index: u32,
        delta: String,
    },
    #[serde(rename = "response.done")]
    ResponseDone { response: serde_json::Value },
    #[serde(rename = "input_audio_buffer.speech_started")]
    InputAudioBufferSpeechStarted {
        audio_start_ms: u32,
        item_id: String,
    },
    #[serde(rename = "input_audio_buffer.speech_stopped")]
    InputAudioBufferSpeechStopped { audio_end_ms: u32, item_id: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Debug)]
pub struct ErrorDetails {
    pub code: Option<String>,
    pub message: String,
    pub param: Option<String>,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    App = {{App}} {
        ui: <Root>{
            main_window = <Window>{
                body = <View>{
                    flow: Down,
                    spacing: 30,
                    align: {
                        x: 0.5,
                        y: 0.5
                    },
                    show_bg: true,
                    draw_bg: {
                        fn pixel(self) -> vec4 {
                            return mix(#2, #5, self.pos.y);
                        }
                    }

                    <Label> {
                        text: "OpenAI Realtime Audio Chat"
                        draw_text: {text_style: {font_size: 24}}
                    }

                    connection_status = <Label> {
                        text: "Disconnected"
                        draw_text: {text_style: {font_size: 16}}
                    }

                    toggle_interruptions = <Toggle> {
                        text: "Allow interruptions (requires headphones, no AEC yet)"
                        draw_text: {text_style: {font_size: 14}}
                        label_walk: {
                            margin: {left: 50}
                        }
                        draw_bg: {
                            size: 25.
                        }
                    }

                    button_connect = <Button> {
                        text: "🔗 Connect to OpenAI"
                        draw_text: {text_style: {font_size: 18}}
                    }

                    button_start_conversation = <Button> {
                        text: "🎤 Start Conversation"
                        draw_text: {text_style: {font_size: 18}}
                    }

                    button_stop_conversation = <Button> {
                        text: "⏹️ Stop Conversation"
                        draw_text: {text_style: {font_size: 18}}
                    }

                    transcript_label = <Label> {
                        width: Fill,
                        padding: {left: 30, right: 30}
                        height: 200
                        draw_text: {text_style: {font_size: 14}}
                    }

                    status_label = <Label> {
                        text: "Ready to connect"
                        draw_text: {text_style: {font_size: 16}}
                    }
                }
            }
        }
    }
}

app_main!(App);

#[derive(Live, LiveHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    recorded_audio: Arc<Mutex<Vec<f32>>>,
    #[rust]
    playback_audio: Arc<Mutex<Vec<f32>>>,
    #[rust]
    is_recording: Arc<Mutex<bool>>,
    #[rust]
    is_playing: Arc<Mutex<bool>>,
    #[rust]
    playback_position: Arc<Mutex<usize>>,
    #[rust]
    audio_setup_done: bool,
    #[rust]
    websocket: Option<WebSocket>,
    #[rust]
    is_connected: bool,
    #[rust]
    conversation_active: bool,
    #[rust]
    current_transcript: String,
    #[rust]
    openai_api_key: Option<String>,
    #[rust]
    audio_streaming_timer: Option<Timer>,
    #[rust]
    has_sent_audio: bool,
    #[rust]
    ai_is_responding: bool,
    #[rust]
    user_is_interrupting: bool,
    #[rust]
    current_assistant_item_id: Option<String>,
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        crate::makepad_widgets::live_design(cx);
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.setup_audio(cx);
        self.update_ui_state(cx);

        self.openai_api_key = std::env::var("OPENAI_API_KEY").ok();
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(id!(button_connect)).clicked(&actions) {
            self.connect_to_openai(cx);
        }

        if self
            .ui
            .button(id!(button_start_conversation))
            .clicked(&actions)
        {
            self.start_conversation(cx);
        }

        if self
            .ui
            .button(id!(button_stop_conversation))
            .clicked(&actions)
        {
            self.stop_conversation(cx);
        }

        if let Some(enabled) = self.ui.check_box(id!(toggle_interruptions)).changed(&actions) {
            if enabled {
                *self.is_recording.lock().unwrap() = true;
            }
        }
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        log!(
            "App::handle_audio_devices called with {} devices",
            devices.descs.len()
        );
        for desc in &devices.descs {
            log!("Audio device: {}", desc);
        }

        // Use default input and output devices
        let default_input = devices.default_input();
        let default_output = devices.default_output();

        log!("Default input: {:?}", default_input);
        log!("Default output: {:?}", default_output);

        cx.use_audio_inputs(&default_input);
        cx.use_audio_outputs(&default_output);
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if let Event::Timer(_timer_event) = event {
            if let Some(audio_timer) = &self.audio_streaming_timer {
                if audio_timer.is_event(event).is_some() {
                    if self.conversation_active {
                        self.send_audio_chunk_to_openai(cx);
                    }

                    // Check if we should resume recording when playback buffer is empty
                    // This is the backup mechanism for when toggle is OFF (no interruptions)
                    if self.playback_audio.lock().unwrap().is_empty() {
                        let interruptions_enabled = self.ui.check_box(id!(toggle_interruptions)).active(cx);
                        
                        if !interruptions_enabled {
                            // Only auto-resume recording if interruptions are disabled
                            // (when interruptions are enabled, recording control is handled elsewhere)
                            if let Ok(mut is_recording) = self.is_recording.try_lock() {
                                if !*is_recording && self.conversation_active && !self.ai_is_responding {
                                    println!("Auto-resuming recording - playback empty and interruptions disabled");
                                    *is_recording = true;
                                    self.ui.label(id!(status_label)).set_text(cx, "🎤 Listening...");
                                }
                            }
                        }
                    }
                }
            }
        }

        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());

        self.handle_websocket_messages(cx);
    }
}

impl App {
    fn setup_audio(&mut self, cx: &mut Cx) {
        if self.audio_setup_done {
            log!("Audio already setup, skipping");
            return;
        }

        let recorded_audio = self.recorded_audio.clone();
        let is_recording = self.is_recording.clone();

        log!("Setting up audio input callback");

        // Audio input callback - capture for OpenAI streaming
        cx.audio_input(0, move |_info, input_buffer| {
            if let Ok(is_recording_guard) = is_recording.try_lock() {
                if *is_recording_guard {
                    if let Ok(mut recorded) = recorded_audio.try_lock() {
                        let channel = input_buffer.channel(0);

                        // Downsample from 48kHz to 24kHz by taking every other sample
                        // TODO: this is a simple decimation - for better quality, we should use proper filtering
                        for i in (0..channel.len()).step_by(2) {
                            recorded.push(channel[i]);
                        }
                    }
                }
            }
        });

        let playback_audio = self.playback_audio.clone();
        let playback_position = self.playback_position.clone();
        let is_playing = self.is_playing.clone();

        // Audio output callback - plays OpenAI response audio
        cx.audio_output(0, move |_info, output_buffer| {
            // Always start with silence
            output_buffer.zero();

            if let Ok(mut playback) = playback_audio.try_lock() {
                if let Ok(mut pos) = playback_position.try_lock() {
                    if let Ok(mut playing) = is_playing.try_lock() {
                        // Check if we should continue playing
                        if *playing && !playback.is_empty() && *pos < playback.len() * 2 {
                            // Write to all output channels (mono -> stereo if needed)
                            let frame_count = output_buffer.frame_count();
                            let channel_count = output_buffer.channel_count();
                            
                            let mut samples_to_drain = 0;

                            for frame_idx in 0..frame_count {
                                // Upsample from 24kHz to 48kHz by duplicating each sample
                                let sample_idx = *pos / 2; // Each 24kHz sample maps to 2 48kHz samples

                                if sample_idx < playback.len() {
                                    let audio_sample = playback[sample_idx];

                                    // Write the same sample to all output channels
                                    for channel_idx in 0..channel_count {
                                        let channel = output_buffer.channel_mut(channel_idx);
                                        channel[frame_idx] = audio_sample;
                                    }

                                    *pos += 1;
                                    
                                    // Track how many samples we can safely remove (every 2 pos increments = 1 sample)
                                    if *pos % 2 == 0 {
                                        samples_to_drain += 1;
                                    }
                                } else {
                                    // Reached end of audio data
                                    *playing = false;
                                    *pos = 0;
                                    // Drain remaining samples since we're done
                                    samples_to_drain = playback.len();
                                    break;
                                }
                            }
                            
                            // Remove consumed samples from the front of the buffer
                            if samples_to_drain > 0 && samples_to_drain <= playback.len() {
                                playback.drain(..samples_to_drain);
                                // Adjust position since we removed samples from the front
                                *pos = (*pos).saturating_sub(samples_to_drain * 2);
                                // log!("Drained {} samples, buffer size now: {}, pos: {}", 
                                //         samples_to_drain, playback.len(), *pos);
                            }
                        } else {
                            // Not playing or no data - ensure we output silence
                            if *playing && playback.is_empty() {
                                *playing = false;
                                *pos = 0;
                            }
                        }
                    }
                }
            }
        });

        self.audio_setup_done = true;
    }

    fn connect_to_openai(&mut self, cx: &mut Cx) {
        if self.openai_api_key.is_none() {
            self.ui
                .label(id!(connection_status))
                .set_text(cx, "❌ Please set OPENAI_API_KEY");
            return;
        }

        // Create WebSocket connection
        let url =
            "wss://api.openai.com/v1/realtime?model=gpt-4o-realtime-preview-2025-06-03".to_string();

        let mut request = HttpRequest::new(url, HttpMethod::GET);
        request.set_header(
            "Authorization".to_string(),
            format!("Bearer {}", self.openai_api_key.as_ref().unwrap()),
        );
        request.set_header("OpenAI-Beta".to_string(), "realtime=v1".to_string());

        self.websocket = Some(WebSocket::open(request));
        self.ui
            .label(id!(connection_status))
            .set_text(cx, "🔄 Connecting...");

        log!("WebSocket connection initiated");
    }

    fn handle_websocket_messages(&mut self, cx: &mut Cx) {
        // Collect messages first to avoid borrowing conflicts
        let mut messages = Vec::new();

        if let Some(websocket) = &mut self.websocket {
            while let Ok(message) = websocket.try_recv() {
                messages.push(message);
            }
        }

        // Process messages
        for message in messages {
            match message {
                WebSocketMessage::Opened => {
                    log!("WebSocket connected to OpenAI");
                    self.is_connected = true;
                    self.ui
                        .label(id!(connection_status))
                        .set_text(cx, "✅ Connected to OpenAI");
                    self.initialize_session(cx);
                    self.update_ui_state(cx);
                }
                WebSocketMessage::String(data) => {
                    // log!("Received WebSocket message: {}", data);
                    self.handle_openai_message(cx, &data);
                }
                WebSocketMessage::Binary(data) => {
                    log!("Received binary WebSocket message: {} bytes", data.len());
                }
                WebSocketMessage::Error(error) => {
                    log!("WebSocket error: {}", error);
                    self.ui
                        .label(id!(connection_status))
                        .set_text(cx, &format!("❌ Error: {}", error));
                    self.is_connected = false;
                    self.update_ui_state(cx);
                }
                WebSocketMessage::Closed => {
                    log!("WebSocket closed");
                    self.ui
                        .label(id!(connection_status))
                        .set_text(cx, "❌ Disconnected");
                    self.is_connected = false;
                    self.conversation_active = false;
                    self.update_ui_state(cx);
                }
            }
        }
    }

    /// Initialize the OpenAI Realtime session with audio configuration
    fn initialize_session(&mut self, _cx: &mut Cx) {
        log!("Initializing OpenAI session");

        let session_config = SessionConfig {
            modalities: vec!["text".to_string(), "audio".to_string()],
            instructions: "You are a helpful AI assistant. Respond naturally and conversationally."
                .to_string(),
            voice: "alloy".to_string(),
            input_audio_format: "pcm16".to_string(),
            output_audio_format: "pcm16".to_string(),
            input_audio_transcription: Some(TranscriptionConfig {
                model: "whisper-1".to_string(),
            }),
            turn_detection: Some(TurnDetectionConfig {
                detection_type: "server_vad".to_string(), // Server-side VAD. Turns are detected by the server.
                threshold: 0.8,
                prefix_padding_ms: 300,
                silence_duration_ms: 500,
                interrupt_response: true,
                create_response: true,
            }),
            tools: vec![],
            tool_choice: "none".to_string(),
            temperature: 0.8,
            max_response_output_tokens: Some(4096),
        };

        let message = OpenAIRealtimeMessage::SessionUpdate {
            session: session_config,
        };

        self.send_openai_message(message);
    }

    fn handle_openai_message(&mut self, cx: &mut Cx, data: &str) {
        match serde_json::from_str::<OpenAIRealtimeResponse>(data) {
            Ok(response) => {
                match response {
                    OpenAIRealtimeResponse::SessionCreated { .. } => {
                        log!("OpenAI session created successfully");
                        self.ui
                            .label(id!(status_label))
                            .set_text(cx, "✅ Session ready");
                        // Update connection status and UI state
                        self.is_connected = true;
                        self.ui
                            .label(id!(connection_status))
                            .set_text(cx, "✅ Connected to OpenAI");
                        self.update_ui_state(cx);
                    }
                    OpenAIRealtimeResponse::SessionUpdated { .. } => {
                        log!("OpenAI session updated successfully");
                        self.ui
                            .label(id!(status_label))
                            .set_text(cx, "✅ Session configured");
                    }
                    OpenAIRealtimeResponse::ResponseAudioDelta { item_id, delta, .. } => {
                        if self.user_is_interrupting {
                            log!("Ignoring AI audio delta - user is interrupting");
                            return;
                        }

                        if self.current_assistant_item_id.is_none() {
                            self.current_assistant_item_id = Some(item_id.clone());
                            log!("Started receiving audio for assistant item ID: {}", item_id);
                        }

                        self.ai_is_responding = true;
                        if self.conversation_active {
                            let interruptions_enabled = self.ui.check_box(id!(toggle_interruptions)).active(cx);
                            
                            if !interruptions_enabled {
                                // Interruptions disabled - mute microphone during AI speech
                                *self.is_recording.lock().unwrap() = false;
                                println!("Setting is_recording to false (interruptions disabled)");
                            }
                        }

                        // Decode base64 audio and add to playback buffer
                        if let Ok(audio_bytes) = general_purpose::STANDARD.decode(&delta) {
                            self.add_audio_to_playback(audio_bytes);
                        }

                        self.ui.label(id!(status_label)).set_text(cx, "🔊 Playing audio...");
                    }
                    OpenAIRealtimeResponse::ResponseAudioTranscriptDelta { delta, .. } => {
                        self.ai_is_responding = true;

                        // Update transcript with AI response
                        self.current_transcript.push_str(&delta);

                        // Keep transcript manageable for demo purposes
                        if self.current_transcript.len() > 500 {
                            let truncated = self
                                .current_transcript
                                .chars()
                                .skip(200)
                                .collect::<String>();
                            self.current_transcript = truncated;
                        }

                        self.ui
                            .label(id!(transcript_label))
                            .set_text(cx, &self.current_transcript);
                    }
                    OpenAIRealtimeResponse::ResponseDone { .. } => {
                        let status_label = self.ui.label(id!(status_label));
                        self.user_is_interrupting = false;
                        self.ai_is_responding = false;
                        self.current_assistant_item_id = None;

                        // Resume recording after AI response is complete
                        if self.conversation_active {
                            // Check if interruptions are enabled via the toggle
                            let interruptions_enabled = self.ui.check_box(id!(toggle_interruptions)).active(cx);
                            
                            if interruptions_enabled {
                                // Allow immediate interruption
                                *self.is_recording.lock().unwrap() = true;
                                status_label.set_text(cx, "✅ Response generated - 🎤 listening again");
                            } else {
                                // Without interruptions, only resume when playback buffer is truly empty
                                if self.playback_audio.lock().unwrap().is_empty() {
                                    println!("Setting is_recording to true - response completed and playback empty");
                                    *self.is_recording.lock().unwrap() = true;
                                    status_label.set_text(cx, "✅ Response generated - 🎤 listening again");
                                } else {
                                    status_label.set_text(cx, "✅ Response generated - 🔊 playing audio");
                                    println!("Playback still active, keeping recording disabled");
                                }
                            }
                        }
                    }
                    OpenAIRealtimeResponse::InputAudioBufferSpeechStarted { .. } => {
                        log!("Speech detected by OpenAI - interrupting AI audio");
                        self.ui
                            .label(id!(status_label))
                            .set_text(cx, "🎤 Speech detected - interrupting AI");

                        // CRITICAL: Clear the playback audio buffer to stop ongoing AI audio
                        // This prevents audio accumulation and feedback loops
                        if let Ok(mut playback) = self.playback_audio.try_lock() {
                            let cleared_samples = playback.len();
                            playback.clear();
                            log!(
                                "Cleared {} audio samples from playback buffer to prevent feedback",
                                cleared_samples
                            );
                        }

                        // Stop current playback and reset position
                        if let Ok(mut is_playing) = self.is_playing.try_lock() {
                            *is_playing = false;
                        }
                        if let Ok(mut position) = self.playback_position.try_lock() {
                            *position = 0;
                        }

                        // Resume recording immediately when user starts speaking
                        if self.conversation_active {
                            *self.is_recording.lock().unwrap() = true;
                        }
                    }
                    OpenAIRealtimeResponse::InputAudioBufferSpeechStopped { .. } => {
                        log!("Speech ended, processing...");
                        self.ui
                            .label(id!(status_label))
                            .set_text(cx, "🤔 Processing...");

                        // Temporarily stop recording while waiting for response
                        if self.conversation_active {
                            *self.is_recording.lock().unwrap() = false;
                        }
                    }
                    OpenAIRealtimeResponse::ConversationItemCreated { .. } => {
                        self.ui
                            .label(id!(status_label))
                            .set_text(cx, "✅ User speech transcribed");
                    }
                    OpenAIRealtimeResponse::ConversationItemTruncated { .. } => {
                        self.ui
                            .label(id!(status_label))
                            .set_text(cx, "✅ AI speech truncated");
                    }
                    OpenAIRealtimeResponse::Error { error } => {
                        log!("OpenAI API error: {:?}", error);
                        self.ui
                            .label(id!(status_label))
                            .set_text(cx, &format!("❌ Error: {}", error.message));

                        // Resume recording on error
                        if self.conversation_active {
                            *self.is_recording.lock().unwrap() = true;
                        }
                    }
                    _ => {
                        log!("Received other OpenAI message type: {:?}", data);
                    }
                }
            }
            Err(e) => {
                log!("Failed to parse OpenAI message: {}", e);
            }
        }
    }

    fn send_openai_message(&mut self, message: OpenAIRealtimeMessage) {
        if let Some(websocket) = &mut self.websocket {
            match serde_json::to_string(&message) {
                Ok(json_str) => {
                    // log!("Sending to OpenAI: {}", json_str);
                    if let Err(_) = websocket.send_string(json_str) {
                        log!("Failed to send message to OpenAI");
                    }
                }
                Err(e) => {
                    log!("Failed to serialize message: {}", e);
                }
            }
        }
    }

    fn start_conversation(&mut self, cx: &mut Cx) {
        if !self.is_connected {
            self.ui
                .label(id!(status_label))
                .set_text(cx, "❌ Not connected to OpenAI");
            return;
        }

        log!("Starting conversation");
        self.conversation_active = true;
        self.ai_is_responding = false;
        *self.is_recording.lock().unwrap() = true;
        self.has_sent_audio = false;

        // Clear previous audio
        self.recorded_audio.lock().unwrap().clear();
        self.playback_audio.lock().unwrap().clear();
        *self.is_playing.lock().unwrap() = false;
        *self.playback_position.lock().unwrap() = 0;
        self.current_transcript.clear();

        self.ui
            .label(id!(status_label))
            .set_text(cx, "🎤 Listening...");
        self.update_ui_state(cx);

        // Start streaming audio immediately
        self.start_audio_streaming(cx);
    }

    fn stop_conversation(&mut self, cx: &mut Cx) {
        log!("Stopping conversation");
        self.conversation_active = false;
        self.ai_is_responding = false;
        *self.is_recording.lock().unwrap() = false;

        // Stop the audio streaming timer
        if let Some(timer) = &self.audio_streaming_timer {
            cx.stop_timer(*timer);
            self.audio_streaming_timer = None;
        }

        // Send final audio chunk and commit if we have sent audio
        self.send_audio_chunk_to_openai(cx);
        if self.has_sent_audio {
            let commit_message = OpenAIRealtimeMessage::InputAudioBufferCommit;
            self.send_openai_message(commit_message);
        }

        self.ui
            .label(id!(status_label))
            .set_text(cx, "⏹️ Conversation stopped");
        self.update_ui_state(cx);
    }

    fn start_audio_streaming(&mut self, cx: &mut Cx) {
        // Start a timer to send audio chunks every 20ms
        let timer = cx.start_interval(0.020);
        self.audio_streaming_timer = Some(timer);
    }

    fn send_audio_chunk_to_openai(&mut self, _cx: &mut Cx) {
        // Collect audio data to avoid borrowing conflicts
        let audio_data = if let Ok(mut recorded) = self.recorded_audio.try_lock() {
            if !recorded.is_empty() {
                let data = recorded.clone();
                recorded.clear();
                Some(data)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(samples) = audio_data {
            // Convert f32 samples to PCM16 bytes
            let pcm16_bytes = self.convert_f32_to_pcm16(&samples);

            // Encode as base64 for transmission
            let base64_audio = general_purpose::STANDARD.encode(&pcm16_bytes);

            let message = OpenAIRealtimeMessage::InputAudioBufferAppend {
                audio: base64_audio,
            };
            self.send_openai_message(message);

            self.has_sent_audio = true;
        }
    }

    fn convert_f32_to_pcm16(&self, samples: &[f32]) -> Vec<u8> {
        let mut pcm16_bytes = Vec::with_capacity(samples.len() * 2);

        for &sample in samples {
            // Clamp to [-1.0, 1.0] and convert to i16
            let clamped = sample.max(-1.0).min(1.0);
            let pcm16_sample = (clamped * 32767.0) as i16;
            pcm16_bytes.extend_from_slice(&pcm16_sample.to_le_bytes());
        }

        pcm16_bytes
    }

    fn add_audio_to_playback(&mut self, audio_bytes: Vec<u8>) {
        // Don't add audio if user is currently speaking (to prevent feedback)
        if !self.ai_is_responding {
            log!("Skipping AI audio - user is speaking or AI not actively responding");
            return;
        }

        // Convert PCM16 bytes back to f32 samples
        let samples = self.convert_pcm16_to_f32(&audio_bytes);

        if let Ok(mut playback) = self.playback_audio.try_lock() {
            // If we're not currently playing, clear the buffer first to avoid accumulation
            if let Ok(mut is_playing) = self.is_playing.try_lock() {
                if !*is_playing {
                    // Clear old audio data and start fresh playback
                    playback.clear();
                    *self.playback_position.lock().unwrap() = 0;
                    *is_playing = true;
                    log!(
                        "Started fresh playback of OpenAI response audio ({} samples)",
                        samples.len()
                    );
                } else {
                    // log!("Appending to existing playback ({} samples)", samples.len());
                }
            }

            playback.extend_from_slice(&samples);
        }
    }

    fn convert_pcm16_to_f32(&self, bytes: &[u8]) -> Vec<f32> {
        let mut samples = Vec::with_capacity(bytes.len() / 2);

        for chunk in bytes.chunks_exact(2) {
            let pcm16_sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            let f32_sample = pcm16_sample as f32 / 32767.0;
            samples.push(f32_sample);
        }

        samples
    }

    fn update_ui_state(&self, cx: &mut Cx) {
        // Update button states based on connection and conversation status
        if !self.is_connected {
            self.ui
                .button(id!(button_connect))
                .set_text(cx, "🔗 Connect to OpenAI");
            self.ui
                .button(id!(button_start_conversation))
                .set_text(cx, "🎤 Start Conversation (Disconnected)");
            self.ui
                .button(id!(button_stop_conversation))
                .set_text(cx, "⏹️ Stop Conversation");
        } else if self.conversation_active {
            self.ui
                .button(id!(button_connect))
                .set_text(cx, "✅ Connected");
            self.ui
                .button(id!(button_start_conversation))
                .set_text(cx, "🎤 Conversation Active");
            self.ui
                .button(id!(button_stop_conversation))
                .set_text(cx, "⏹️ Stop Conversation");
        } else {
            self.ui
                .button(id!(button_connect))
                .set_text(cx, "✅ Connected");
            self.ui
                .button(id!(button_start_conversation))
                .set_text(cx, "🎤 Start Conversation");
            self.ui
                .button(id!(button_stop_conversation))
                .set_text(cx, "⏹️ Stop Conversation");
        }
    }
}
