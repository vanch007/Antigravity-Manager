# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Antigravity Tools 是一个基于 Tauri v2 的桌面应用,提供 AI 账号管理和 API 协议反代服务。将 Web 端 Session (Google/Anthropic) 转化为标准化的 OpenAI 兼容 API 接口。

**技术栈:**
- 前端: React 19 + TypeScript + Vite + TailwindCSS + DaisyUI
- 后端: Rust + Tauri v2 + Axum (反代服务器)
- 状态管理: Zustand
- 国际化: i18next
- 数据库: SQLite (rusqlite)

## 常用命令

### 开发环境
```bash
# 安装依赖
npm install

# 启动开发服务器 (前端 + Tauri)
npm run dev

# 仅构建前端
npm run build

# 启动 Tauri 开发模式 (带调试日志)
npm run tauri:debug

# 构建生产版本
npm run tauri build
```

### Rust 后端开发
```bash
# 进入 Rust 项目目录
cd src-tauri

# 编译检查
cargo check

# 运行测试
cargo test

# 构建 release 版本
cargo build --release

# 运行 headless 模式 (无 GUI,仅代理服务)
cargo run -- --headless
```

### Docker 部署
```bash
# 构建 Docker 镜像
docker build -t antigravity-tools .

# 运行容器 (headless 模式)
docker run -p 8045:8045 -e API_KEY=your-key antigravity-tools
```

## 架构设计

### 双进程架构

**前端 (React)**:
- 路径: `src/`
- 职责: UI 界面、用户交互、账号管理界面
- 与后端通信: Tauri Commands (IPC)

**后端 (Rust)**:
- 路径: `src-tauri/src/`
- 职责:
  - Tauri 窗口管理和系统集成
  - 内嵌 Axum HTTP 服务器 (API 反代)
  - 账号管理、配额计算、智能调度
  - 协议转换 (Claude/Gemini ↔ OpenAI)

### 核心模块结构

#### 后端模块 (`src-tauri/src/`)

```
lib.rs              # 应用入口,支持 GUI 和 headless 模式
├── commands/       # Tauri Commands (前端调用的 Rust 函数)
│   ├── account.rs  # 账号 CRUD
│   ├── proxy.rs    # 代理服务启动/停止
│   └── ...
├── modules/        # 业务逻辑模块
│   ├── account.rs      # 账号管理核心
│   ├── quota.rs        # 配额计算与刷新
│   ├── process.rs      # Claude 批量刷新引擎
│   ├── scheduler.rs    # 智能调度器 (定时任务)
│   ├── oauth.rs        # OAuth 流程
│   ├── token_stats.rs  # Token 统计
│   ├── cloudflared.rs  # Cloudflare Tunnel 集成
│   └── ...
├── proxy/          # API 反代服务 (Axum 服务器)
│   ├── server.rs       # Axum 路由和服务器主逻辑
│   ├── token_manager.rs # 账号选择、负载均衡、失败重试
│   ├── mappers/        # 协议转换层
│   │   ├── claude/     # Claude API ↔ OpenAI 协议转换
│   │   ├── gemini/     # Gemini API ↔ OpenAI 协议转换
│   │   ├── openai/     # OpenAI 流式响应处理
│   │   └── ...
│   ├── handlers/       # API 端点处理器
│   │   ├── chat.rs     # /v1/chat/completions
│   │   ├── models.rs   # /v1/models
│   │   ├── audio.rs    # /v1/audio/*
│   │   └── ...
│   ├── upstream/       # 上游 API 客户端
│   ├── middleware/     # CORS, 日志, 认证
│   └── ...
└── models/         # 数据模型 (Serde 序列化)
    ├── config.rs   # 配置结构体
    ├── quota.rs    # 配额模型
    └── token.rs    # Token 统计模型
```

#### 前端模块 (`src/`)

```
App.tsx             # 根组件和路由
├── pages/          # 页面组件
│   ├── Dashboard.tsx   # 仪表盘
│   ├── Accounts.tsx    # 账号管理
│   ├── ApiProxy.tsx    # API 代理设置
│   ├── Settings.tsx    # 系统设置
│   └── ...
├── components/     # 可复用组件
│   ├── accounts/   # 账号相关组件
│   ├── common/     # 通用组件 (Toast, Modal 等)
│   ├── layout/     # 布局组件
│   └── ...
├── stores/         # Zustand 状态管理
│   ├── useAccountStore.ts
│   ├── useProxyStore.ts
│   └── ...
├── services/       # API 服务层 (调用 Tauri Commands)
│   └── tauri.ts
└── types/          # TypeScript 类型定义
```

### 关键数据流

#### 1. API 请求处理流程
```
客户端 HTTP 请求
  → Axum 中间件 (认证/CORS/日志)
  → handlers/* (解析 OpenAI 格式请求)
  → token_manager (智能选择账号)
  → mappers/* (转换为 Claude/Gemini 协议)
  → upstream/* (调用上游 API)
  → mappers/* (转换响应为 OpenAI 格式)
  → 流式返回客户端
```

#### 2. 账号配额刷新流程
```
scheduler.rs 定时触发
  → process.rs (批量并发刷新)
  → account.rs (获取会话详情)
  → quota.rs (计算剩余配额)
  → 更新 SQLite 数据库
  → 前端通过 WebSocket/Polling 更新 UI
```

#### 3. OAuth 登录流程
```
前端触发登录
  → oauth.rs 启动临时 HTTP 服务器
  → 打开浏览器 (Tauri opener)
  → 用户授权后回调到本地服务器
  → 提取 session token
  → 保存到数据库
  → 关闭临时服务器
```

## 重要设计模式

### 1. 协议映射器 (Protocol Mappers)
- 位置: `src-tauri/src/proxy/mappers/`
- 职责: 双向转换 OpenAI ↔ Claude/Gemini 协议
- 关键文件:
  - `claude/models.rs`: Claude 数据结构定义
  - `gemini/models.rs`: Gemini 数据结构定义
  - `openai/streaming.rs`: SSE 流式响应封装

### 2. Token Manager (智能调度)
- 位置: `src-tauri/src/proxy/token_manager.rs`
- 功能:
  - 账号池管理
  - 负载均衡 (轮询/随机/配额优先)
  - 失败重试和降级
  - 粘性会话 (对话保持同一账号)
  - 配额保护 (预留阈值)

### 3. 会话指纹管理
- 位置: `src-tauri/src/proxy/session_manager.rs`
- 功能: 为每个对话会话分配固定账号,保证上下文连续性

### 4. 双模式运行
- **GUI 模式**: 完整的 Tauri 桌面应用
- **Headless 模式**: `--headless` 启动,仅运行反代服务器,适合 Docker 部署

## 配置与环境变量

### 配置文件
- 路径: `~/.antigravity/gui_config.json` (macOS/Linux)
- 路径: `%APPDATA%/antigravity/gui_config.json` (Windows)

### 环境变量 (Headless 模式)
```bash
API_KEY / ABV_API_KEY          # API 认证密钥
WEB_PASSWORD / ABV_WEB_PASSWORD # Web UI 管理密码
RUST_LOG=debug                 # 日志级别
```

## 开发注意事项

### 前端开发
- Vite 开发服务器运行在 `localhost:1420`
- API 代理配置: `/api/*` → `http://127.0.0.1:8045`
- Tauri Commands 通过 `@tauri-apps/api` 调用
- 使用 `i18next` 处理多语言,所有文本需国际化

### 后端开发
- Axum 服务器默认监听 `127.0.0.1:8045`
- 使用 `tracing` 进行日志记录,避免 `println!`
- 异步运行时: Tokio
- 数据库操作使用 `rusqlite`,连接池在 `modules/db.rs`

### 协议转换开发
- 新增模型映射: 修改 `proxy/common/model_mapping.rs`
- 新增 API 端点: 在 `proxy/handlers/` 添加处理器,并在 `server.rs` 注册路由
- 流式响应必须使用 `async-stream` 和 `tokio_stream`

### 测试
- 单元测试: `cargo test`
- 集成测试: `src-tauri/src/proxy/tests/`
- 手动测试反代服务: 启动后使用 `curl` 或 Postman 测试 `/v1/chat/completions`

## 调试技巧

### 查看日志
```bash
# macOS/Linux
tail -f ~/.antigravity/logs/antigravity.log

# 或使用 RUST_LOG 环境变量
RUST_LOG=debug npm run tauri dev
```

### 数据库检查
```bash
sqlite3 ~/.antigravity/gui_config.db
.schema
SELECT * FROM accounts;
```

### 前端调试
- 使用浏览器 DevTools (Tauri 开发窗口支持)
- Zustand DevTools: 在 `stores/` 中启用

### 代理调试
- 查看请求日志: `proxy/middleware/logging.rs`
- 测试上游连接: `proxy/upstream/client.rs` 包含重试逻辑
- 监控流式传输: 检查 `mappers/openai/streaming.rs`

## 构建与发布

### 版本号管理
同时更新以下文件中的版本号:
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### 构建平台包
```bash
# macOS (DMG + App Bundle)
npm run tauri build

# Windows (MSI + EXE)
npm run tauri build -- --target x86_64-pc-windows-msvc

# Linux (AppImage + deb)
npm run tauri build -- --target x86_64-unknown-linux-gnu
```

### Docker 镜像
```bash
cd docker
docker build -t antigravity-tools:latest .
```

## 性能优化

### 后端优化
- 连接池: Axum 使用共享的 HTTP 客户端 (`upstream/client.rs`)
- 批量刷新: `process.rs` 使用并发限制避免过载
- 缓存: 签名缓存 (`signature_cache.rs`) 减少重复计算

### 前端优化
- 虚拟滚动: 大量账号列表使用 `@tanstack/react-virtual`
- 防抖: 配置项变更使用 `DebouncedSlider` 组件
- 懒加载: 路由级别的代码分割

## 安全考虑

- Session Token 存储在本地 SQLite,使用文件权限保护
- API Key 验证在 `proxy/middleware/` 中间件
- CORS 严格配置,默认仅允许本地访问
- Admin 密码单独配置,可与 API Key 分离
