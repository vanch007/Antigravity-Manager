# Antigravity Tools (2API 版本) 🚀

<div align="center">
  <img src="public/icon.png" alt="Antigravity Logo" width="120" height="120" style="border-radius: 24px; box-shadow: 0 10px 30px rgba(0,0,0,0.15);">

  <h3>不仅仅是账号管理，更是您的个人 AI 网关</h3>
  <p>完美代理 Gemini & Claude，兼容 OpenAI 协议，打破调用限制。</p>
  
  <p>
    <a href="https://github.com/lbjlaq/Antigravity-Manager">
      <img src="https://img.shields.io/badge/Version-3.0.0-blue?style=flat-square" alt="Version">
    </a>
    <img src="https://img.shields.io/badge/Tauri-v2-orange?style=flat-square" alt="Tauri">
    <img src="https://img.shields.io/badge/React-18-61DAFB?style=flat-square" alt="React">
    <img src="https://img.shields.io/badge/License-CC--BY--NC--SA--4.0-lightgrey?style=flat-square" alt="License">
  </p>

  <p>
    <a href="#-Downloads">📥 下载最新版</a> • 
    <a href="#-API-Proxy">🔌 API 反代 (新!)</a> • 
    <a href="#-Features">✨ 账号管理</a>
  </p>

  <p>
    <strong>🇨🇳 简体中文</strong> | 
    <a href="./README_EN.md">🇺🇸 English</a>
  </p>
</div>

---

**Antigravity Tools 2API** 次世代版本发布！这不仅仅是一个账号管理器，它将您的桌面变成了一个强大的 **本地 AI 网关 (Local AI Gateway)**。

通过内置的高性能 Rust 反代服务，您可以将浏览器中的 Web Session (`sid`, `__Secure-1PSID` 等) 转化为标准的 **OpenAI API** 接口。这意味着您可以在 **Cursor**, **Windsurf**, **LangChain**, **NextChat** 等任何支持 OpenAI 协议的应用中，无缝调用 Gemini 和 Claude 的高级模型能力。

> **寻找旧版文档?**
> v2.0 纯账号管理版本的文档已移动至 [README_v2.md](./README_v2.md)。

## � 核心亮点：API 反代 (API Proxy)

将 Web 账号战力转化为标准 API 生产力！

- **OpenAI 协议兼容**: 提供标准的 `/v1/chat/completions` 和 `/v1/models` 接口，无缝对接所有生态应用。
- **多模型支持**:
    - **Google**: `gemini-2.0-flash-exp`, `gemini-1.5-pro`
    - **Claude**: `claude-3-5-sonnet-20241022`
- **智能轮询 (Auto-Rotation)**: 添加多个账号后，系统会自动在配额耗尽或触发风控时切换到下一个可用账号，实现近乎无限的调用体验。
- **视觉模型**: 完整支持 GPT-4o 格式的图片输入，自动转换为 Gemini 视觉协议。

### 🖼️ 能力展示 (Showcase)

<div align="center">

| **Gemini 3 Pro Image (Imagen 3)** | **Claude 3.5 Sonnet (Thinking)** |
| :---: | :---: |
| <img src="docs/images/v3/gemini-image-edit.jpg" width="100%" style="border-radius: 8px;"> | <img src="docs/images/v3/claude-code-gen.png" width="100%" style="border-radius: 8px;"> |
| **NextChat - 图像编辑/生成** | **Windsurf/Cursor - 复杂代码生成** |

</div>

## ✨ 经典功能：账号管理

- **Token 自动保活**: 自动刷新过期 Token，确保随时可用。
- **可视化配额**:
    - **文本额度**: 精确显示 Gemini Pro / Claude 3.5 Sonnet 剩余百分比。
    - **图片额度 (新)**: 新增 Gemini Image (Vision) 额度监控，绘图/识图不再盲目。
- **IDE 注入**: 一键将 Token 注入到本地 VSCode / Cursor 数据库，无需手动填写 Header。
- **托盘常驻**: 极简托盘菜单，随时查看核心指标。

## � 快速开始

### 1. 添加账号
在 **"账号列表"** 页面，通过 OAuth 登录或手动粘贴 Token 添加您的 Google/Anthropic 账号。

### 2. 启动服务
进入 **"API 反代"** 页面：
1. 配置端口 (默认 8045)。
2. 点击 **"启动服务"**。
3. 复制生成的 **API Key** (默认为 `sk-antigravity`)。

### 3. 连接使用
在任何 AI 应用中配置：
- **Base URL**: `http://localhost:8045/` (部分应用可能需要填写 `http://localhost:8045/v1`)
- **Key**: `sk-antigravity` (任意不为空的字符串)
- **Model**: 请使用以下支持的模型 ID

#### 📚 支持的模型列表 (Supported Models)

| 模型 ID | 说明 |
| :--- | :--- |
| **gemini-2.5-flash** | **Flash 2.5**。极速响应，超高性价比。 |
| **gemini-2.5-flash-thinking** | **Flash Thinking**。具备思考能力的轻量级模型。 |
| **gemini-3-pro-high** | **Gemini 3 Pro**。Google 最强 reasoning 模型。 |
| **gemini-3-pro-low** | **Gemini 3 Pro (Low)**。低配额消耗版。 |
| **gemini-3-pro-image** | **Imagen 3**。绘图专用模型。 |
| **claude-sonnet-4-5** | **Claude 3.5 Sonnet**。代码与逻辑推理首选。 |
| **claude-sonnet-4-5-thinking** | **Sonnet Thinking**。开启了思维链的 Sonnet。 |
| **claude-opus-4-5-thinking** | **Opus Thinking**。Claude 最强思维模型。 |

> 💡 **提示**: 反代服务支持透传所有 Google/Anthropic 官方模型 ID，您可以直接使用官方文档中的任何模型名称。

## �️ 技术栈升级

| 模块 | 旧版 v2.0 | 新版 v2.2 (2API) |
| :--- | :--- | :--- |
| **API Server** | N/A | **Rust (Axum)** 高并发异步服务 |
| **HTTP Client** | Reqwest | **Reqwest + EventSource** 流式响应 |
| **Format** | N/A | **OpenAI <-> Gemini** 实时协议转换 |

## 📄 版权说明

Copyright © 2025 Antigravity. 
本项目采用 **[CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/)** 协议许可。
仅供个人学习研究使用，禁止用于商业用途。
