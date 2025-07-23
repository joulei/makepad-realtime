# OpenAI Realtime API Demo

A basic integration with OpenAI's Realtime API for conversational audio, built with Makepad.

## Features

- Real-time audio streaming (24kHz PCM16)
- WebSocket connection to OpenAI with authentication
- Server-side Voice Activity Detection (VAD)
- Audio interruption handling
- Full duplex audio (simultaneous recording and playback)

## Usage

1. **Set your OpenAI API key:**
   ```bash
   export OPENAI_API_KEY="your-api-key-here"
   ```

2. **Run the demo:**
   ```bash
   cargo run
   ```

3. **Use the interface:**
   - Click "Connect to OpenAI" to establish connection
   - Click "Start Conversation" to begin audio chat
   - Speak naturally - the AI will respond with voice
   - Click "Stop Conversation" when done

## Requirements

- OpenAI API key with Realtime API access
- Microphone and speakers/headphones
- Rust toolchain

## Limitations

- Makpead is not able to use my airpods as audio input correctly (reads empty audio), but it outputs audio to them fine.
- Only tested in macOS, other platforms in Makepad have some limitations in their websocket support.

Many of these and more will be fixed and properly implemented in MolyKit. 
