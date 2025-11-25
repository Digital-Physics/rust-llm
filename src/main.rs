use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

const CACHE_FILE: &str = ".file_watcher_cache.json";

#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileSnapshot {
    content: String,
    modified: SystemTime,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Cache {
    snapshots: HashMap<String, FileSnapshot>,
}

impl Cache {
    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(CACHE_FILE, json);
        }
    }

    fn update_snapshot(&mut self, path: &str, content: String, modified: SystemTime) {
        self.snapshots.insert(
            path.to_string(),
            FileSnapshot { content, modified },
        );
    }

    fn get_snapshot(&self, path: &str) -> Option<&FileSnapshot> {
        self.snapshots.get(path)
    }
}

fn should_ignore(path: &Path) -> bool {
    let ignore_list = [
        "target",
        ".git",
        "node_modules",
        ".cache",
        CACHE_FILE,
        "README.md",
    ];

    path.components().any(|c| {
        if let Some(s) = c.as_os_str().to_str() {
            ignore_list.contains(&s)
        } else {
            false
        }
    })
}

fn get_tracked_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.flatten() {
            let path = entry.path();
            if !should_ignore(&path) {
                if path.is_file() {
                    files.push(path);
                } else if path.is_dir() {
                    collect_files_recursive(&path, &mut files);
                }
            }
        }
    }
    files
}

fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !should_ignore(&path) {
                if path.is_file() {
                    files.push(path);
                } else if path.is_dir() {
                    collect_files_recursive(&path, files);
                }
            }
        }
    }
}

// Helper function to normalize paths to relative format
fn normalize_path(path: &Path) -> String {
    if path.is_absolute() {
        if let Ok(current_dir) = std::env::current_dir() {
            if let Ok(rel_path) = path.strip_prefix(&current_dir) {
                return format!("./{}", rel_path.to_string_lossy());
            }
        }
    }
    path.to_string_lossy().to_string()
}

fn initialize_cache() -> Cache {
    println!("📄 Initializing cache with current file state...");
    let mut cache = Cache::default();
    
    for file_path in get_tracked_files() {
        if let Ok(content) = fs::read_to_string(&file_path) {
            if let Ok(metadata) = fs::metadata(&file_path) {
                if let Ok(modified) = metadata.modified() {
                    let path_str = normalize_path(&file_path);
                    cache.update_snapshot(&path_str, content.clone(), modified);
                    println!("📸 Cached: {} ({} bytes)", path_str, content.len());
                }
            }
        }
    }
    
    cache.save();
    println!("✅ Cache initialized with current state (baseline set).");
    cache
}

fn calculate_diff(old_content: &str, new_content: &str) -> String {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();

    // Build a set of old lines for quick lookup
    let old_set: std::collections::HashSet<&str> = old_lines.iter().copied().collect();
    let new_set: std::collections::HashSet<&str> = new_lines.iter().copied().collect();

    let mut diff_output = String::new();

    // Find lines that were removed (in old but not in new)
    for line in &old_lines {
        if !new_set.contains(line) {
            diff_output.push_str(&format!("-{}\n", line));
        }
    }

    // Find lines that were added (in new but not in old)
    for line in &new_lines {
        if !old_set.contains(line) {
            diff_output.push_str(&format!("+{}\n", line));
        }
    }

    diff_output
}

fn process_changes(cache: &mut Cache, paths: &HashSet<PathBuf>, processing: Arc<Mutex<bool>>) {
    // Check if already processing
    {
        let mut is_processing = processing.lock().unwrap();
        if *is_processing {
            println!("⏸️  Already processing changes, skipping...");
            return;
        }
        *is_processing = true;
    }

    let mut diff_content = String::new();

    for path in paths {
        let path_str = normalize_path(path);

        if let Ok(new_content) = fs::read_to_string(path) {
            println!("🔍 Checking file: {}", path_str);
            
            if let Some(snapshot) = cache.get_snapshot(&path_str) {
                println!("📋 Cached content length: {}", snapshot.content.len());
                println!("📋 New content length: {}", new_content.len());
            } else {
                println!("📋 No snapshot found in cache!");
            }
            
            let filtered_diff = if let Some(snapshot) = cache.get_snapshot(&path_str) {
                calculate_diff(&snapshot.content, &new_content)
            } else {
                // New file - show all lines as additions
                new_content.lines()
                    .map(|line| format!("+{}", line))
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            if !filtered_diff.trim().is_empty() {
                diff_content.push_str(&format!("File: {:?}\n", path_str));
                diff_content.push_str(&filtered_diff);
                diff_content.push_str("\n");

                // Update cache
                if let Ok(metadata) = fs::metadata(path) {
                    if let Ok(modified) = metadata.modified() {
                        cache.update_snapshot(&path_str, new_content, modified);
                    }
                }
            }
        }
    }

    if !diff_content.trim().is_empty() {
        println!("🔍 ---------------- DEBUG START ----------------");
        println!("Content being sent to AI:");
        println!("{}", diff_content);
        println!("🔍 ---------------- DEBUG END ------------------");

        send_to_llm_and_update_readme(&diff_content, cache);
    } else {
        println!("ℹ️  No actual changes detected after filtering.");
    }

    // Release processing lock
    {
        let mut is_processing = processing.lock().unwrap();
        *is_processing = false;
    }
}

#[tokio::main]
async fn send_to_llm_and_update_readme(diff_content: &str, cache: &mut Cache) {
    let client = reqwest::Client::new();

    // This is very detailed; we may want a simpler prompt.
    let prompt = format!(
        "
        You are an automated technical writer maintaining a project's README. \
        You will receive a git diff of recent changes. \
        Your job is to write a concise, engaging log entry for these changes. \
        1. Use Markdown. \
        2. Use emojis to categorize changes (e.g., 🐛 for bugs, ✨ for features, ⚡ for perf). \
        3. Include short code snippets in backticks ` ` if relevant. \
        4. Lines starting with '+'/'-' are added/removed lines of code.
        5. Do NOT write introductions like 'Here is the summary'. Just write the content. 
        \n\n{}",
        diff_content
    );

    let payload = serde_json::json!({
        // "model": "qwen2.5-coder:1.5b", // this model had issues
        "model": "qwen2.5-coder:7b", // this model performs better but slower (should test/quantify)
        "prompt": prompt,
        "stream": false,
        "context": [], //don't want previous conversation context
        "options": {
            "num_ctx": 1024,
            "temperature": 0.3,
            "num_predict": 50
        }
    });

    println!("🧠 Sending diff ({} bytes) to Ollama...", diff_content.len());

    match client
        .post("http://localhost:11434/api/generate")
        .timeout(Duration::from_secs(30))  // Add timeout
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                if let Some(message_content) = json["response"].as_str() {
                    println!("📝 LLM Response: {}", message_content);
                    append_to_readme(message_content);
                    cache.save();
                    println!("✅ README updated successfully!");
                } else {
                    eprintln!("❌ Failed to parse LLM response");
                }
            }
        }
        Err(e) => eprintln!("❌ Failed to call Ollama: {}", e),
    }
}

fn append_to_readme(summary: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("### 📅 Update: {}\n{}\n", timestamp, summary.trim());

    let readme_path = "README.md";
    let existing_content = fs::read_to_string(readme_path).unwrap_or_default();

    let new_content = if existing_content.trim().is_empty() {
        format!("# Project Updates\n\n{}", entry)
    } else {
        format!("{}\n{}", existing_content, entry)
    };

    fs::write(readme_path, new_content).expect("Failed to write to README.md");
    println!("✨ content appended to README.md");
}

fn main() {
    // Initialize cache with current state (reset/rebase)
    let mut cache = initialize_cache();

    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher =
        Watcher::new(tx, Config::default()).expect("Failed to create watcher");

    watcher
        .watch(Path::new("."), RecursiveMode::Recursive)
        .expect("Failed to watch directory");

    println!("👀 Watching current directory...");

    let mut pending_changes: HashSet<PathBuf> = HashSet::new();
    let mut last_event_time = SystemTime::now();
    let debounce_duration = Duration::from_secs(4);
    let processing = Arc::new(Mutex::new(false));

    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(Event {
                kind: EventKind::Modify(_) | EventKind::Create(_),
                paths,
                ..
            })) => {
                for path in paths {
                    if !should_ignore(&path) && path.is_file() {
                        println!("📝 Change detected: {:?}", path);
                        pending_changes.insert(path);
                        last_event_time = SystemTime::now();
                    }
                }
            }
            Ok(Err(e)) => eprintln!("❌ Watch error: {:?}", e),
            Err(_) => {
                // Timeout - check if debounce period has elapsed
                if !pending_changes.is_empty() {
                    if let Ok(elapsed) = last_event_time.elapsed() {
                        if elapsed >= debounce_duration {
                            println!("⏳ Debounce finished. Calculating diffs...");
                            process_changes(&mut cache, &pending_changes, Arc::clone(&processing));
                            pending_changes.clear();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}