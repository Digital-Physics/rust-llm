use anyhow::{Context, Result};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use tokio::time::sleep;

// Configuration
const OLLAMA_URL: &str = "http://127.0.0.1:11434/api/chat";
const MODEL_NAME: &str = "qwen2.5-coder:7b";
const DEBOUNCE_DURATION: u64 = 2; // Seconds to wait for silence before processing

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatMessageContent,
}

#[derive(Deserialize)]
struct ChatMessageContent {
    content: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting README Updater...");
    println!("📡 Connecting to Ollama at {} with model {}", OLLAMA_URL, MODEL_NAME);
    println!("👀 Watching current directory for changes...");

    // 1. Setup File Watcher
    // We use a channel to communicate between the watcher thread and our main loop
    let (tx, rx) = channel();
    
    // Initialize the watcher
    let mut watcher = RecommendedWatcher::new(tx, Config::default())
        .context("Failed to create file watcher")?;

    // Watch the current directory recursively
    watcher.watch(Path::new("."), RecursiveMode::Recursive)
        .context("Failed to watch directory")?;

    let client = Client::new();
    let mut last_event_time = Instant::now();
    let mut needs_processing = false;

    // 2. Main Event Loop
    // We poll for events and implement manual debouncing
    loop {
        // Check for new filesystem events
        // We use try_recv to not block, so we can handle the timeout logic below
        while let Ok(res) = rx.try_recv() {
            match res {
                Ok(event) => {
                    // Filter out events for .git folder and README.md to avoid loops
                    let should_ignore = event.paths.iter().any(|p| {
                        let s = p.to_string_lossy();
                        s.contains(".git") || s.ends_with("README.md")
                    });

                    if !should_ignore {
                        println!("📝 Change detected: {:?}", event.kind);
                        last_event_time = Instant::now();
                        needs_processing = true;
                    }
                }
                Err(e) => println!("⚠️ Watch error: {:?}", e),
            }
        }

        // Debounce logic:
        // If we have pending changes AND enough time has passed since the last event...
        if needs_processing && last_event_time.elapsed() > Duration::from_secs(DEBOUNCE_DURATION) {
            println!("⏳ Debounce finished. Analyzing changes...");
            
            // Reset state immediately
            needs_processing = false; 

            // Execute the update pipeline
            if let Err(e) = process_changes(&client).await {
                eprintln!("❌ Error processing changes: {:?}", e);
            } else {
                println!("✅ README updated successfully!");
            }
        }

        // Sleep briefly to prevent high CPU usage in this poll loop
        sleep(Duration::from_millis(100)).await;
    }
}

async fn process_changes(client: &Client) -> Result<()> {
    // 1. Run git diff
    // We explicitly exclude README.md from the diff to prevent the AI from analyzing its own previous output.
    let output = Command::new("git")
        .args(["diff", ".", ":(exclude)README.md"]) 
        .output()
        .context("Failed to execute git diff")?;

    let diff_text = String::from_utf8_lossy(&output.stdout);

    if diff_text.trim().is_empty() {
        println!("🤷 No significant code changes found in diff.");
        return Ok(());
    }

    println!("🧠 Sending diff ({} bytes) to Ollama...", diff_text.len());

    // 2. Construct the Prompt
    let system_prompt = "You are an automated technical writer maintaining a project's README. \
        You will receive a git diff of recent changes. \
        Your job is to write a concise, engaging log entry for these changes. \
        1. Use Markdown. \
        2. Use emojis to categorize changes (e.g., 🐛 for bugs, ✨ for features, ⚡ for perf). \
        3. Include short code snippets in backticks ` ` if relevant. \
        4. Do NOT write introductions like 'Here is the summary'. Just write the content.";

    let user_prompt = format!("Analyze this diff and write a summary entry:\n\n```diff\n{}\n```", diff_text);

    // 3. Call Ollama API
    let request = ChatRequest {
        model: MODEL_NAME.to_string(),
        messages: vec![
            ChatMessage { role: "system".to_string(), content: system_prompt.to_string() },
            ChatMessage { role: "user".to_string(), content: user_prompt },
        ],
        stream: false,
    };

    let res = client.post(OLLAMA_URL)
        .json(&request)
        .send()
        .await
        .context("Failed to send request to Ollama")?;

    if !res.status().is_success() {
        let err_text = res.text().await?;
        return Err(anyhow::anyhow!("Ollama API Error: {}", err_text));
    }

    let chat_res: ChatResponse = res.json().await.context("Failed to parse Ollama response")?;
    let generated_text = chat_res.message.content;

    // 4. Update README.md
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("\n\n### 📅 Update: {}\n{}\n", timestamp, generated_text);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("README.md")
        .context("Failed to open README.md")?;

    file.write_all(entry.as_bytes()).context("Failed to write to README.md")?;

    println!("✨ content appended to README.md");
    Ok(())
}