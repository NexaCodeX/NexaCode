use nexacode_core::llm::{LLMClient, ProviderConfig, ChatOptions, Message};
use futures::StreamExt;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <API_KEY> [BASE_URL]", args[0]);
        eprintln!("Example: {} sk-... https://api.openai.com/v1", args[0]);
        std::process::exit(1);
    }
    
    let api_key = &args[1];
    let base_url = args.get(2).map(|s| s.as_str());
    
    println!("=== LLM Integration Test ===\n");
    
    // Create client
    let mut config = ProviderConfig::openai(api_key);
    if let Some(url) = base_url {
        config = config.with_base_url(url);
    }
    
    let client = match LLMClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {}", e);
            std::process::exit(1);
        }
    };
    
    println!("Provider: {}", client.provider_name());
    
    // Test 1: List models
    println!("\n[Test 1] Listing models...");
    match client.list_models().await {
        Ok(models) => {
            println!("✓ Found {} models:", models.len());
            for model in models.iter().take(5) {
                println!("  - {}", model.id);
            }
            if models.len() > 5 {
                println!("  ... and {} more", models.len() - 5);
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to list models: {}", e);
        }
    }
    
    // Test 2: Simple chat
    println!("\n[Test 2] Simple chat...");
    let messages = vec![
        Message::user("Say 'Hello, World!' and nothing else."),
    ];
    
    let options = ChatOptions::new("glm-5")
        .with_max_tokens(50);
    
    match client.chat(messages.clone(), options).await {
        Ok(response) => {
            println!("✓ Response received:");
            println!("  Model: {}", response.model);
            println!("  Content: {}", response.content);
            if let Some(usage) = response.usage {
                println!("  Tokens: {} prompt + {} completion = {} total",
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
            }
        }
        Err(e) => {
            eprintln!("✗ Chat failed: {}", e);
        }
    }
    
    // Test 3: Streaming chat
    println!("\n[Test 3] Streaming chat...");
    let messages = vec![
        Message::user("Count from 1 to 5, one number per line."),
    ];
    
    let options = ChatOptions::new("glm-5")
        .with_max_tokens(100)
        .with_stream(true);
    
    match client.chat_stream(messages, options).await {
        Ok(mut stream) => {
            println!("✓ Stream started:");
            print!("  ");
            
            let mut full_response = String::new();
            let mut chunk_count = 0;
            
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        chunk_count += 1;
                        if !chunk.delta.is_empty() {
                            print!("{}", chunk.delta);
                            full_response.push_str(&chunk.delta);
                        }
                        if chunk.finish_reason.is_some() {
                            println!();
                            println!("  [Finished: {:?}]", chunk.finish_reason);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("\n✗ Stream error: {}", e);
                        break;
                    }
                }
            }
            
            println!("  Total chunks: {}", chunk_count);
            println!("  Full response length: {} chars", full_response.len());
        }
        Err(e) => {
            eprintln!("✗ Stream failed: {}", e);
        }
    }
    
    println!("\n=== Test Complete ===");
}
