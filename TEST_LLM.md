# LLM Integration Test

## 快速测试

### 方法 1: 运行测试示例（推荐）

```bash
# 使用 OpenAI API
cargo run --manifest-path crates/nexacode-core/Cargo.toml --example test_llm -- "sk-your-api-key"

# 使用自定义 base URL（如 OpenRouter、本地模型等）
cargo run --manifest-path crates/nexacode-core/Cargo.toml --example test_llm -- "sk-your-api-key" "https://openrouter.ai/api/v1"

# 使用 Ollama 本地模型
cargo run --manifest-path crates/nexacode-core/Cargo.toml --example test_llm -- "ollama" "http://localhost:11434/v1"
```

### 方法 2: 运行单元测试

```bash
# 设置环境变量
export OPENAI_API_KEY="sk-your-api-key"

# 运行测试
cargo test --manifest-path crates/nexacode-core/Cargo.toml -- --nocapture
```

## 测试内容

测试会验证以下功能：

1. **列出模型** - 调用 `/v1/models` API
2. **普通对话** - 发送消息并等待完整响应
3. **流式对话** - 实时接收响应流

## 预期输出

```
=== LLM Integration Test ===

Provider: OpenAI

[Test 1] Listing models...
✓ Found 50 models:
  - gpt-4
  - gpt-4o
  - gpt-4-turbo
  - gpt-3.5-turbo
  - gpt-3.5-turbo-16k
  ... and 45 more

[Test 2] Simple chat...
✓ Response received:
  Model: gpt-4o-mini
  Content: Hello, World!
  Tokens: 12 prompt + 5 completion = 17 total

[Test 3] Streaming chat...
✓ Stream started:
  1
  2
  3
  4
  5
  [Finished: Some("stop")]
  Total chunks: 8
  Full response length: 11 chars

=== Test Complete ===
```

## 常见问题

### 问题: 没有收到响应

**可能原因**:
1. API key 无效
2. Base URL 错误
3. 模型名称不存在
4. 网络连接问题

**解决方法**:
- 检查 API key 是否正确
- 确认 base URL 是否正确（注意要包含 `/v1`）
- 验证模型名称是否存在于该 provider

### 问题: 流式响应为空

**可能原因**:
- 流式解析逻辑有问题（已在最新版本修复）

**解决方法**:
- 确保使用最新代码
- 查看测试输出的 chunk 数量

## 支持的 Provider

| Provider | API Key | Base URL |
|----------|---------|----------|
| OpenAI | `sk-...` | `https://api.openai.com/v1` |
| Anthropic | `sk-ant-...` | `https://api.anthropic.com/v1` |
| OpenRouter | `sk-or-...` | `https://openrouter.ai/api/v1` |
| Ollama | `ollama` | `http://localhost:11434/v1` |
| vLLM | `EMPTY` | `http://localhost:8000/v1` |
