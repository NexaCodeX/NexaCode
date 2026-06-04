# NexaCode

<div align="center">

**一个基于 AI 的现代化代码编辑器**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-19-61dafb.svg)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-6.0-3178c6.svg)](https://www.typescriptlang.org/)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)

</div>

## 📖 项目简介

NexaCode 是一个基于 **Tauri 2.0** + **React 19** + **Rust** 构建的现代化 AI 代码编辑器桌面应用。它结合了 Web 技术的灵活性和 Rust 的高性能，为开发者提供智能化的代码编写体验。

### ✨ 核心特性

- 🤖 **AI 驱动** - 集成 AI 助手，提供智能代码补全、重构建议和代码解释
- ⚡ **高性能** - 基于 Tauri 和 Rust，启动快速，内存占用低
- 🎨 **现代化 UI** - 使用 React 19 构建，界面美观流畅
- 📝 **多语言支持** - 支持多种编程语言的语法高亮和智能提示
- 🔧 **可扩展** - 模块化架构，易于扩展和定制
- 💻 **跨平台** - 支持 Windows、macOS 和 Linux

## 🛠️ 技术栈

### 前端
- **React 19** - 现代化 UI 框架
- **TypeScript 6.0** - 类型安全的 JavaScript
- **Vite 8** - 极速构建工具
- **Sass** - CSS 预处理器
- **React Markdown** - Markdown 渲染
- **Syntax Highlighter** - 代码语法高亮

### 后端
- **Tauri 2.0** - 轻量级桌面应用框架
- **Rust 2021** - 系统级编程语言
- **Tokio** - 异步运行时
- **Reqwest** - HTTP 客户端

## 📦 项目结构

```
NexaCode/
├── src/                      # React 前端源码
│   ├── components/          # React 组件
│   ├── hooks/               # 自定义 Hooks
│   ├── utils/               # 工具函数
│   └── App.tsx              # 主应用组件
├── crates/                   # Rust crates
│   ├── nexacode-core/       # 核心业务逻辑
│   └── nexacode-desktop/    # Tauri 桌面应用入口
├── dist/                     # 前端构建输出
├── target/                   # Rust 构建输出
├── package.json             # Node.js 依赖配置
├── Cargo.toml               # Rust workspace 配置
└── tauri.conf.json          # Tauri 配置文件
```

## 🚀 快速开始

### 环境要求

- **Node.js** >= 18
- **Rust** >= 1.70
- **pnpm/npm/yarn**

### 安装依赖

```bash
# 安装前端依赖
npm install

# Rust 依赖会在首次运行时自动安装
```

### 开发模式

```bash
# 启动开发服务器（带热重载）
npm run tauri:dev
```

### 构建发布

```bash
# 构建生产版本
npm run tauri:build
```

构建产物位于 `src-tauri/target/release/bundle/` 目录下。

## 📝 开发脚本

| 命令 | 说明 |
|------|------|
| `npm run dev` | 启动 Vite 开发服务器 |
| `npm run build` | 构建前端资源 |
| `npm run lint` | 运行 ESLint 检查 |
| `npm run tauri:dev` | 启动 Tauri 开发模式 |
| `npm run tauri:build` | 构建生产版本 |

## 🤝 贡献指南

欢迎贡献代码、报告问题或提出建议！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 提交 Pull Request

## 📄 许可证

本项目基于 [MIT License](LICENSE) 开源。

## 🔗 相关链接

- [GitHub 仓库](https://github.com/fansili/NexaCode)
- [Tauri 官方文档](https://tauri.app/)
- [React 官方文档](https://react.dev/)
- [Rust 官方文档](https://www.rust-lang.org/)

---

<div align="center">

Made with ❤️ by NexaCode Team

</div>