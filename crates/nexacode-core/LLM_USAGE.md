# LLM Integration Usage Examples

## Basic Usage

### OpenAI
```rust
use nexacode_core::llm::{LLMClient, Message, ChatOptions, ProviderConfig};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create OpenAI client
    let config = ProviderConfig::openai("sk-your-api-key");
    let client = LLMClient::new(config)?;
    
    // Prepare messages
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Hello, how are you?"),
    ];
    
    // Chat options
    let options = ChatOptions::new("gpt-4")
        .with_temperature(0.7)
        .with_max_tokens(1000);
    
    // Send chat request
    let response = client.chat(messages, options).await?;
    println!("Response: {}", response.content);
    
    Ok(())
}
```

### Anthropic Claude
```rust
use nexacode_core::llm::{LLMClient, Message, ChatOptions, ProviderConfig};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create Anthropic client
    let config = ProviderConfig::anthropic("sk-ant-your-api-key");
    let client = LLMClient::new(config)?;
    
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Explain quantum computing in simple terms."),
    ];
    
    let options = ChatOptions::new("claude-3-5-sonnet-20241022");
    
    let response = client.chat(messages, options).await?;
    println!("Response: {}", response.content);
    
    Ok(())
}
```

### Streaming Response
```rust
use nexacode_core::llm::{LLMClient, Message, ChatOptions, ProviderConfig};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = ProviderConfig::openai("sk-your-api-key");
    let client = LLMClient::new(config)?;
    
    let messages = vec![
        Message::user("Tell me a story about a brave knight."),
    ];
    
    let options = ChatOptions::new("gpt-4").with_stream(true);
    
    // Get streaming response
    let mut stream = client.chat_stream(messages, options).await?;
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        print!("{}", chunk.delta);
        
        if let Some(reason) = chunk.finish_reason {
            println!("\nFinished: {}", reason);
            break;
        }
    }
    
    Ok(())
}
```

### OpenAI-Compatible Services (Ollama, vLLM, etc.)
```rust
use nexacode_core::llm::{LLMClient, ProviderConfig};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Ollama example
    let config = ProviderConfig::openai_compatible(
        "ollama",  // or any API key
        "http://localhost:11434/v1"
    )
    .with_default_model("llama2");
    
    let client = LLMClient::new(config)?;
    
    // Use the same way as OpenAI
    // ...
    
    Ok(())
}
```

### Multi-modal (with images)
```rust
use nexacode_core::llm::{LLMClient, Message, ChatOptions, ProviderConfig, ImageContent};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = ProviderConfig::openai("sk-your-api-key");
    let client = LLMClient::new(config)?;
    
    let messages = vec![
        Message::user("What's in this image?")
            .with_images(vec![
                ImageContent {
                    url: "https://example.com/image.jpg".to_string(),
                    detail: None,
                }
            ]),
    ];
    
    let options = ChatOptions::new("gpt-4-vision-preview");
    let response = client.chat(messages, options).await?;
    
    Ok(())
}
```

### List Available Models
```rust
use nexacode_core::llm::{LLMClient, ProviderConfig};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = ProviderConfig::openai("sk-your-api-key");
    let client = LLMClient::new(config)?;
    
    let models = client.list_models().await?;
    for model in models {
        println!("Model: {}", model.id);
    }
    
    Ok(())
}
```

## Tauri Integration

In your Tauri backend (`nexacode-desktop`):

```rust
use tauri::command;
use nexacode_core::llm::{LLMClient, Message, ChatOptions, ProviderConfig};

#[command]
async fn chat_with_ai(
    api_key: String,
    model: String,
    prompt: String,
) -> Result<String, String> {
    let config = ProviderConfig::openai(&api_key);
    let client = LLMClient::new(config).map_err(|e| e.to_string())?;
    
    let messages = vec![Message::user(&prompt)];
    let options = ChatOptions::new(&model);
    
    let response = client.chat(messages, options)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(response.content)
}
```

## Configuration Storage

You can serialize/deserialize provider configs:

```rust
use nexacode_core::llm::ProviderConfig;

// Save to file
let config = ProviderConfig::openai("sk-your-key");
let json = serde_json::to_string(&config)?;

// Load from file
let loaded: ProviderConfig = serde_json::from_str(&json)?;
```
