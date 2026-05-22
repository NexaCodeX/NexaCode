use nexacode_core::llm::{LLMClient, ProviderConfig, ChatOptions, Message};

#[tokio::test]
async fn test_openai_chat() {
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("Please set OPENAI_API_KEY environment variable");
    
    let config = ProviderConfig::openai(api_key);
    let client = LLMClient::new(config).expect("Failed to create client");
    
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Say 'Hello, World!' and nothing else."),
    ];
    
    let options = ChatOptions::new("gpt-4o-mini")
        .with_max_tokens(50);
    
    println!("Sending chat request...");
    let response = client.chat(messages, options).await.expect("Chat failed");
    
    println!("Response: {}", response.content);
    println!("Model: {}", response.model);
    
    assert!(!response.content.is_empty(), "Response should not be empty");
}

#[tokio::test]
async fn test_openai_stream() {
    use futures::StreamExt;
    
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("Please set OPENAI_API_KEY environment variable");
    
    let config = ProviderConfig::openai(api_key);
    let client = LLMClient::new(config).expect("Failed to create client");
    
    let messages = vec![
        Message::user("Count from 1 to 5, one number per line."),
    ];
    
    let options = ChatOptions::new("gpt-4o-mini")
        .with_max_tokens(100)
        .with_stream(true);
    
    println!("Sending streaming request...");
    let mut stream = client.chat_stream(messages, options).await.expect("Stream failed");
    
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
                    println!("\n[Finished: {:?}]", chunk.finish_reason);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
    
    println!("\nTotal chunks: {}", chunk_count);
    println!("Full response: {}", full_response);
    
    assert!(!full_response.is_empty(), "Response should not be empty");
    assert!(chunk_count > 1, "Should receive multiple chunks");
}

#[tokio::test]
async fn test_list_models() {
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("Please set OPENAI_API_KEY environment variable");
    
    let config = ProviderConfig::openai(api_key);
    let client = LLMClient::new(config).expect("Failed to create client");
    
    println!("Listing models...");
    let models = client.list_models().await.expect("Failed to list models");
    
    println!("Available models:");
    for model in &models {
        println!("  - {} ({:?})", model.id, model.name);
    }
    
    assert!(!models.is_empty(), "Should have at least one model");
}
