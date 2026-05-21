# OpenAI Compatible Models Configuration

NexaCode 支持所有兼容 OpenAI API 的本地和云端模型服务。

## 支持的服务

### 1. Ollama
```toml
[providers.ollama]
config = { 
  provider_type = "openai_compatible", 
  api_key = "ollama", 
  base_url = "http://localhost:11434/v1", 
  default_model = "llama2" 
}
is_active = true
```

**常用模型**：
- `llama2` - Llama 2
- `llama3` - Llama 3
- `mistral` - Mistral
- `codellama` - Code Llama
- `deepseek-coder` - DeepSeek Coder

### 2. vLLM
```toml
[providers.vllm]
config = { 
  provider_type = "openai_compatible", 
  api_key = "EMPTY", 
  base_url = "http://localhost:8000/v1", 
  default_model = "your-model-name" 
}
is_active = false
```

### 3. LM Studio
```toml
[providers.lmstudio]
config = { 
  provider_type = "openai_compatible", 
  api_key = "lm-studio", 
  base_url = "http://localhost:1234/v1", 
  default_model = "local-model" 
}
is_active = false
```

### 4. Text Generation WebUI (oobabooga)
```toml
[providers.textgen]
config = { 
  provider_type = "openai_compatible", 
  api_key = "textgen", 
  base_url = "http://localhost:5000/v1", 
  default_model = "model-name" 
}
is_active = false
```

### 5. OpenAI API 代理/镜像
```toml
[providers.openai-proxy]
config = { 
  provider_type = "openai", 
  api_key = "sk-your-key", 
  base_url = "https://your-proxy.com/v1", 
  default_model = "gpt-4" 
}
is_active = false
```

### 6. Azure OpenAI
```toml
[providers.azure]
config = { 
  provider_type = "openai", 
  api_key = "your-azure-key", 
  base_url = "https://your-resource.openai.azure.com/openai/deployments/your-deployment", 
  default_model = "gpt-4" 
}
is_active = false
```

### 7. Anthropic API 代理
```toml
[providers.claude-proxy]
config = { 
  provider_type = "anthropic", 
  api_key = "sk-ant-your-key", 
  base_url = "https://your-proxy.com/v1", 
  default_model = "claude-3-5-sonnet-20241022" 
}
is_active = false
```

## 配置说明

### Base URL 字段

- **OpenAI 类型**：可选，用于设置代理或镜像站点
- **Anthropic 类型**：可选，用于设置代理或镜像站点
- **OpenAI Compatible 类型**：必填，本地服务的地址

### 常见端口

| 服务 | 默认端口 | Base URL |
|------|---------|----------|
| Ollama | 11434 | `http://localhost:11434/v1` |
| vLLM | 8000 | `http://localhost:8000/v1` |
| LM Studio | 1234 | `http://localhost:1234/v1` |
| TextGen | 5000 | `http://localhost:5000/v1` |

### API Key

- **本地服务**：可以是任意字符串（如 "ollama", "EMPTY"）
- **云端服务**：需要真实的 API key

## 使用建议

1. **本地开发**：使用 Ollama 或 LM Studio，无需 API key
2. **生产环境**：使用 vLLM 部署，性能更好
3. **代理服务**：设置 base URL 指向你的代理服务器
4. **多模型切换**：配置多个 provider，随时切换

## 示例配置文件

完整的 `~/.nexacode/config.toml` 示例：

```toml
[providers.openai]
config = { provider_type = "openai", api_key = "sk-...", default_model = "gpt-4" }
is_active = false

[providers.claude]
config = { provider_type = "anthropic", api_key = "sk-ant-...", default_model = "claude-3-5-sonnet-20241022" }
is_active = false

[providers.ollama]
config = { provider_type = "openai_compatible", api_key = "ollama", base_url = "http://localhost:11434/v1", default_model = "llama3" }
is_active = true

[providers.vllm]
config = { provider_type = "openai_compatible", api_key = "EMPTY", base_url = "http://localhost:8000/v1", default_model = "mistral-7b" }
is_active = false
```
