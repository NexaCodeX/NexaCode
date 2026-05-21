# NexaCode LLM Integration - Quick Start

## 功能概述

NexaCode 现已集成完整的 LLM 交互功能，支持与 OpenAI、Anthropic Claude 以及 OpenAI 兼容服务（如 Ollama）进行对话。

## 配置文件

配置文件保存在：`~/.nexacode/config.toml`

示例配置：
```toml
[providers.openai]
config = { provider_type = "openai", api_key = "sk-your-api-key", default_model = "gpt-4" }
is_active = true

[providers.claude]
config = { provider_type = "anthropic", api_key = "sk-ant-your-api-key", default_model = "claude-3-5-sonnet-20241022" }
is_active = false
```

## 使用步骤

### 1. 启动应用

```bash
# 开发模式
npm run tauri dev

# 或者构建生产版本
npm run tauri build
```

### 2. 配置 LLM Provider

首次启动时，应用会自动打开设置面板。你需要：

1. 点击 **"Add Provider"** 按钮
2. 填写配置信息：
   - **Name**: 自定义名称（如 "my-openai"）
   - **Type**: 选择提供商类型
     - `OpenAI` - OpenAI 官方 API
     - `Anthropic` - Claude API
     - `OpenAI Compatible` - 兼容 OpenAI API 的服务（如 Ollama）
   - **API Key**: 你的 API 密钥
   - **Base URL**: 仅 OpenAI Compatible 类型需要（如 `http://localhost:11434/v1`）
   - **Default Model**: 默认模型名称

3. 点击 **"Add Provider"** 保存

### 3. 开始对话

1. 在主界面底部的输入框中输入你的问题
2. 选择模型（GPT-4、Claude 3.5 Sonnet 等）
3. 点击发送按钮或按 `Enter` 键
4. AI 的回复会以流式方式实时显示

### 4. 管理对话

- **New Chat**: 点击侧边栏的 "New Chat" 按钮开始新对话
- **Settings**: 点击侧边栏底部的 "Settings" 按钮管理 Provider
- **切换 Provider**: 在设置面板中点击 "Set Active" 切换当前使用的 Provider

## 支持的功能

✅ **流式响应** - 实时显示 AI 回复  
✅ **多轮对话** - 保持对话上下文  
✅ **多 Provider 管理** - 配置和切换不同的 LLM 服务  
✅ **模型选择** - 每次对话可选择不同模型  
✅ **错误处理** - 友好的错误提示和恢复机制  

## 示例配置

### OpenAI
```
Name: openai
Type: OpenAI
API Key: sk-...
Default Model: gpt-4
```

### Anthropic Claude
```
Name: claude
Type: Anthropic
API Key: sk-ant-...
Default Model: claude-3-5-sonnet-20241022
```

### Ollama (本地)
```
Name: ollama
Type: OpenAI Compatible
API Key: ollama (任意值)
Base URL: http://localhost:11434/v1
Default Model: llama2
```

### vLLM (本地)
```
Name: vllm
Type: OpenAI Compatible
API Key: EMPTY (任意值)
Base URL: http://localhost:8000/v1
Default Model: your-model-name
```

### LM Studio (本地)
```
Name: lmstudio
Type: OpenAI Compatible
API Key: lm-studio (任意值)
Base URL: http://localhost:1234/v1
Default Model: local-model
```

### OpenAI 代理
```
Name: openai-proxy
Type: OpenAI
API Key: sk-...
Base URL: https://your-proxy.com/v1 (可选)
Default Model: gpt-4
```

**💡 提示**：所有 OpenAI 兼容的服务都支持自定义 Base URL，详见 [OPENAI_COMPATIBLE.md](./OPENAI_COMPATIBLE.md)

## 技术架构

- **后端**: Rust + Tauri + nexacode-core
- **前端**: React + TypeScript
- **通信**: Tauri IPC + 事件流
- **状态管理**: React Hooks

## 下一步

- 添加代码高亮显示
- 支持 Markdown 渲染
- 添加对话历史持久化
- 支持文件附件
- 添加更多 LLM Provider
