# Rust + Ollama (qwen2.5-coder:7b) are used to uptate this README.md fil

```
cargo run --release
```

After running that, diffs on tracked git files that get updated will be analyzed by an LLM, and this README.md file gets updated automatically. The update below represents a change to the shopping_list.txt file

### 📅 Update: 2025-11-22 11:27:53
- Adjusted the quantity of bananas from `* 50` to `* 75` 🍌

# Either we're sending the LLM too much, and there is a disconnect, or it has some cached history context. The update below is not based on a one line update in the main file, and it is not hullicinated either.


### 📅 Update: 2025-11-24 19:30:39
The provided code snippet is a Rust program that automates the process of updating a project's README.md file based on changes detected in the repository. The program uses a language model (LLM) to generate summaries of these changes and appends them to the README. Here's a breakdown of how it works:

1. **Initialization**:
   - The program initializes a cache with the current state of the files being tracked.
   - It sets up a file watcher to monitor changes in the project directory.

2. **Change Detection**:
   - When a change is detected (file created or modified), the program records the path of the changed file and marks it for processing.
   - It also records the time of the last event to track when the debounce period starts.

3. **Debouncing**:
   - After a change is detected, the program waits for a predefined debounce duration (in this case, 4 seconds).
   - If no more changes are detected within this duration, it processes all pending changes together.

4. **Processing Changes**:
   - The program calculates the differences between the cached content and the current state of each file.
   - It sends these differences to a language model for processing.
   - The language model generates a summary of the changes.
   - This summary is then appended to the README.md file.

5. **Updating Cache**:
   - After updating the README, the program updates the cache with the new content of the files.

Here's a more detailed breakdown of some key functions and their roles:

- `initialize_cache`: Initializes the cache with the current state of the files being tracked.
- `process_changes`: Calculates the differences between the cached content and the current state of each file, sends these differences to the language model for processing, and appends the summary to the README.md file.
- `send_to_llm_and_update_readme`: Sends the differences to a language model for processing and updates the README.md file with the generated summary.
- `append_to_readme`: Appends the generated summary to the README.md file.

The program uses the `reqwest` library to send requests to the language model's API, and the `git` command to calculate differences between the files. The `chrono` library is used for timestamping entries in the README.

### 📅 Update: 2025-11-24 19:30:49
🎨 **Feature: Enhanced Debug Logging** 🚀 

In this commit, we enhanced our debugging process by adding more detailed logs to help us better understand how data is being sent and processed. This enhancement includes updating the `send_data` function in `src/main.rs`, which now prints out the actual message being sent along with its debug information. This will be particularly useful for debugging issues where the output from the model does not match what we expect.


### 📅 Update: 2025-11-24 19:41:11
This code is a Rust program that integrates with an LLM (likely GPT-3 or similar) to automatically generate and append summaries of recent changes in a project's codebase to its README.md file. Here's a breakdown of the key components:

1. **File Watching**: The program uses `notify` crate to watch for file modifications (`modify`) or new files (`create`) within the current directory.

2. **Debouncing**: To avoid sending too many requests to the LLM, it implements debouncing logic. If changes are detected, they are stored in a set and processed after a certain timeout (default is 4 seconds).

3. **Sending Changes to LLM**: When a debounce period elapses, the program constructs a prompt with the diffs of recent changes and sends it to an LLM API endpoint (`http://localhost:11434/api/generate`). The prompt is formatted in Markdown and includes categories for different types of changes.

4. **Generating Summaries**: The LLM generates a summary based on the provided diffs, which is then appended to the `README.md` file along with a timestamp.

5. **Updating README**: The summary is formatted as a Markdown block under a new heading (`### 📅 Update:`). If the README is empty, it starts with a header (`# Project Updates`).

6. **Error Handling**: The program includes error handling for file operations and LLM API calls.

7. **Initialization**: When starting, the program initializes a cache of current file states to track changes effectively.

This script automates the process of maintaining an up-to-date `README.md` with summaries of project changes, potentially saving time that would otherwise be spent manually updating the documentation.
