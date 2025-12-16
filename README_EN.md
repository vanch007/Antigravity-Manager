# Antigravity Tools (2API Edition) 🚀

<div align="center">
  <img src="public/icon.png" alt="Antigravity Logo" width="120" height="120" style="border-radius: 24px; box-shadow: 0 10px 30px rgba(0,0,0,0.15);">

  <h3>Your Personal API Gateway for Infinite AI</h3>
  <p>Seamlessly proxy Gemini & Claude. OpenAI-Compatible. Privacy First.</p>
  
  <p>
    <a href="https://github.com/lbjlaq/Antigravity-Manager">
      <img src="https://img.shields.io/badge/Version-3.0.0-blue?style=flat-square" alt="Version">
    </a>
    <img src="https://img.shields.io/badge/Tauri-v2-orange?style=flat-square" alt="Tauri">
    <img src="https://img.shields.io/badge/React-18-61DAFB?style=flat-square" alt="React">
    <img src="https://img.shields.io/badge/License-CC--BY--NC--SA--4.0-lightgrey?style=flat-square" alt="License">
  </p>

  <p>
    <a href="#-Downloads">📥 Download</a> • 
    <a href="#-Features">✨ Account Manager</a> • 
    <a href="#-API-Proxy">🔌 API Proxy</a>
  </p>

  <p>
    <strong>🇺🇸 English</strong> | 
    <a href="./README_v2.md">🇨🇳 简体中文 (Legacy v2)</a>
  </p>
</div>

---

**Antigravity Tools 2.2** is a robust desktop application that transforms your desktop into a powerful **Local AI Gateway**.

It not only manages your Gemini / Claude accounts but also provides a **local OpenAI-compatible API server**. This allows you to use your browser-based Google/Claude sessions (`sid`, `__Secure-1PSID`, etc.) as standard API keys in any AI application (Cursor, Windsurf, LangChain, etc.).

> **Looking for the Account Manager Only version?**
> The v2.0 Account Manager documentation has been moved to [README_v2.md](./README_v2.md).

## ✨ Key Features

### 🔌 Local API Proxy (New in 2API)
Turn your browser cookies into a standard OpenAI API!
- **OpenAI-Compatible**: Provides a `/v1/chat/completions` endpoint.
- **Multi-Model Support**:
    - **Gemini**: `gemini-3-pro-high`, `gemini-3-pro-image`, `gemini-2.0-flash-exp`, etc.
    - **Claude**: `claude-sonnet-4-5`
- **Auto-Rotation**: Automatically rotates through your added accounts when rate limits are hit.
- **Image Support**: Full support for vision models (GPT-4o compatible input).

### 🖼️ Capability Showcase

<div align="center">

| **Gemini 3 Pro Image (Imagen 3)** | **Claude 3.5 Sonnet (Thinking)** |
| :---: | :---: |
| <img src="docs/images/v3/gemini-image-edit.jpg" width="100%" style="border-radius: 8px;"> | <img src="docs/images/v3/claude-code-gen.png" width="100%" style="border-radius: 8px;"> |
| **NextChat - Image Gen/Edit** | **Windsurf/Cursor - Complex Coding** |

</div>

### 👥 Account Manager
- **Token Management**: Manage dozens of Gemini/Claude accounts.
- **Auto-Refresh**: Keeps your tokens alive automatically.
- **Quota Monitoring**: Real-time visualization of model quotas (Text & Image).
- **IDE Injection**: Auto-inject tokens into local VSCode-based IDEs (Cursor/Windsurf) for seamless "Pro" usage.

### 🛡️ Privacy First
- **Local Storage**: All data inside `gui_config.json` and `antigravity.db` stays on your machine.
- **No Cloud**: We do not run any intermediary servers. Your data goes directly from your machine to Google/Anthropic.

## 🛠️ Technology Stack

| Component | Tech |
| :--- | :--- |
| **Core** | Rust (Tauri v2) |
| **API Server** | Axum (Rust) |
| **Frontend** | React + TailwindCSS |
| **Database** | SQLite + JSON |

## 📦 Usage

1. **Add Accounts**: Login via OAuth or paste tokens in the "Accounts" tab.
2. **Start Proxy**: Go to "API Proxy" tab and click **Start Service**.
3. **Connect**: 
   - Base URL: `http://localhost:8045/` (Some apps need `http://localhost:8045/v1`)
   - API Key: `sk-antigravity` (Any string)
   - Model: Select from the list below:

#### 📚 Supported Models

| Model ID | Description |
| :--- | :--- |
| **gemini-2.5-flash** | **Flash 2.5**. Extremely fast and cost-effective. |
| **gemini-2.5-flash-thinking** | **Flash Thinking**. Lightweight model with reasoning capabilities. |
| **gemini-3-pro-high** | **Gemini 3 Pro**. Google's strongest reasoning model. |
| **gemini-3-pro-low** | **Gemini 3 Pro (Low)**. Lower quota consumption version. |
| **gemini-3-pro-image** | **Imagen 3**. Dedicated image generation model. |
| **claude-sonnet-4-5** | **Claude 3.5 Sonnet**. Top choice for coding and logic. |
| **claude-sonnet-4-5-thinking** | **Sonnet Thinking**. Sonnet with chain-of-thought enabled. |
| **claude-opus-4-5-thinking** | **Opus Thinking**. Claude's most powerful thinking model. |

> 💡 **Tip**: The proxy supports pass-through for all official Google/Anthropic model IDs.

## 📄 License
CC BY-NC-SA 4.0
