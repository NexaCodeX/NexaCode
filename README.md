# NexaCode

A desktop application built with **Tauri 2.0 + React + TypeScript**.

## Tech Stack

- 🦀 **Rust** (Tauri backend)
- ⚛️ **React 19** (frontend)
- ⚡ **Vite** (build tool)
- 📘 **TypeScript**

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (1.77.2+)
- [Node.js](https://nodejs.org/) (18+)
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)

### Development

```bash
# Install dependencies
npm install

# Start dev server (with Tauri window)
npm run tauri dev
```

### Build

```bash
# Build for production
npm run tauri build
```

## Project Structure

```
NexaCode/
├── src/                    # React frontend
│   ├── App.tsx             # Main app component
│   ├── App.css             # App styles
│   ├── main.tsx            # Entry point
│   └── index.css           # Global styles
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── lib.rs          # Tauri commands & app setup
│   │   └── main.rs         # Entry point
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri configuration
├── index.html
├── package.json
└── vite.config.ts
```

## License

MIT
