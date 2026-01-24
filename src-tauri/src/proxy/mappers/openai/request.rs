// OpenAI → Gemini 请求转换
use super::models::*;
use super::streaming::get_thought_signature;
use serde_json::{json, Value};

pub fn transform_openai_request(
    request: &OpenAIRequest,
    project_id: &str,
    mapped_model: &str,
) -> Value {
    // 将 OpenAI 工具转为 Value 数组以便探测
    let tools_val = request
        .tools
        .as_ref()
        .map(|list| list.iter().map(|v| v.clone()).collect::<Vec<_>>());

    let mapped_model_lower = mapped_model.to_lowercase();

    // Resolve grounding config
    let config = crate::proxy::mappers::common_utils::resolve_request_config(
        &request.model,
        &mapped_model_lower,
        &tools_val,
        request.size.as_deref(),    // [NEW] Pass size parameter
        request.quality.as_deref()  // [NEW] Pass quality parameter
    );

    // 检测 Gemini 3 Pro thinking 模型
    let is_gemini_3_thinking = mapped_model_lower.contains("gemini-3")
        && (mapped_model_lower.ends_with("-high")
            || mapped_model_lower.ends_with("-low")
            || mapped_model_lower.contains("-pro"));
    let is_claude_thinking = mapped_model_lower.ends_with("-thinking");
    let is_thinking_model = is_gemini_3_thinking || is_claude_thinking;

    // [NEW] 检查历史消息是否兼容思维模型 (是否有 Assistant 消息缺失 reasoning_content)
    let has_incompatible_assistant_history = request.messages.iter().any(|msg| {
        msg.role == "assistant"
            && msg
                .reasoning_content
                .as_ref()
                .map(|s| s.is_empty())
                .unwrap_or(true)
    });

    // 获取全局存储的思维签名
    let global_thought_sig = get_thought_signature();

    // [NEW] 决定是否开启 Thinking 功能:
    // 如果是 Claude 思考模型且历史不兼容且没有可用签名来占位, 则禁用 Thinking 以防 400
    let mut actual_include_thinking = is_thinking_model;
    if is_claude_thinking && has_incompatible_assistant_history && global_thought_sig.is_none() {
        tracing::warn!("[OpenAI-Thinking] Incompatible assistant history detected for Claude thinking model without global signature. Disabling thinking for this request to avoid 400 error.");
        actual_include_thinking = false;
    }

    tracing::debug!(
        "[Debug] OpenAI Request: original='{}', mapped='{}', type='{}', has_image_config={}",
        request.model,
        mapped_model,
        config.request_type,
        config.image_config.is_some()
    );

    // 1. 提取所有 System Message 并注入补丁
    let mut system_instructions: Vec<String> = request
        .messages
        .iter()
        .filter(|msg| msg.role == "system" || msg.role == "developer")
        .filter_map(|msg| {
            msg.content.as_ref().map(|c| match c {
                OpenAIContent::String(s) => s.clone(),
                OpenAIContent::Array(blocks) => blocks
                    .iter()
                    .filter_map(|b| {
                        if let OpenAIContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            })
        })
        .collect();

    // [NEW] 如果请求中包含 instructions 字段，优先使用它
    if let Some(inst) = &request.instructions {
        if !inst.is_empty() {
            system_instructions.insert(0, inst.clone());
        }
    }

    // Pre-scan to map tool_call_id to function name (for Codex)
    let mut tool_id_to_name = std::collections::HashMap::new();
    for msg in &request.messages {
        if let Some(tool_calls) = &msg.tool_calls {
            for call in tool_calls {
                let name = &call.function.name;
                let final_name = if name == "local_shell_call" {
                    "shell"
                } else {
                    name
                };
                tool_id_to_name.insert(call.id.clone(), final_name.to_string());
            }
        }
    }

    // 从全局存储获取 thoughtSignature 支持
    let global_thought_sig = get_thought_signature();
    if global_thought_sig.is_some() {
        tracing::debug!(
            "从全局存储获取到 thoughtSignature (长度: {})",
            global_thought_sig.as_ref().unwrap().len()
        );
    }

    // [New] 预先构建工具名称到原始 Schema 的映射，用于后续参数类型修正
    let mut tool_name_to_schema = std::collections::HashMap::new();
    if let Some(tools) = &request.tools {
        for tool in tools {
            if let (Some(name), Some(params)) = (
                tool.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str()),
                tool.get("function").and_then(|f| f.get("parameters")),
            ) {
                tool_name_to_schema.insert(name.to_string(), params.clone());
            } else if let (Some(name), Some(params)) = (
                tool.get("name").and_then(|v| v.as_str()),
                tool.get("parameters"),
            ) {
                // 处理某些客户端可能透传的精简格式
                tool_name_to_schema.insert(name.to_string(), params.clone());
            }
        }
    }

    // 2. 构建 Gemini contents (过滤掉 system/developer 指令)
    let contents: Vec<Value> = request
        .messages
        .iter()
        .filter(|msg| msg.role != "system" && msg.role != "developer")
        .map(|msg| {
            let role = match msg.role.as_str() {
                "assistant" => "model",
                "tool" | "function" => "user", 
                _ => &msg.role,
            };

            let mut parts = Vec::new();

            // Handle reasoning_content (thinking)
            if let Some(reasoning) = &msg.reasoning_content {
                if !reasoning.is_empty() {
                    let mut thought_part = json!({
                        "text": reasoning,
                        "thought": true,
                    });
                    if let Some(ref sig) = global_thought_sig {
                        thought_part["thoughtSignature"] = json!(sig);
                    }
                    parts.push(thought_part);
                }
            } else if actual_include_thinking && role == "model" {
                // [FIX] 解决 Claude 3.7 Thinking 模型的强制性校验:
                // "Expected thinking... but found tool_use/text"
                // 如果是思维模型且缺失 reasoning_content, 则注入占位符
                tracing::debug!("[OpenAI-Thinking] Injecting placeholder thinking block for assistant message");
                let mut thought_part = json!({
                    "text": "Applying tool decisions and generating response...",
                    "thought": true,
                });
                
                // [NEW] 优先使用全局存储的思维签名 (如果可用)
                if let Some(ref sig) = global_thought_sig {
                    thought_part["thoughtSignature"] = json!(sig);
                } else if !mapped_model.starts_with("projects/") && mapped_model.contains("gemini") {
                    // [FIX] 仅针对 Gemini 思维模型注入跳过标签, Claude 不识别此标签
                    thought_part["thoughtSignature"] = json!("skip_thought_signature_validator");
                }
                
                parts.push(thought_part);
            }

            // Handle content (multimodal or text)
            // [FIX] Skip standard content mapping for tool/function roles to avoid duplicate parts
            // These are handled below in the "Handle tool response" section.
            let is_tool_role = msg.role == "tool" || msg.role == "function";
            if let (Some(content), false) = (&msg.content, is_tool_role) {
                match content {
                    OpenAIContent::String(s) => {
                        if !s.is_empty() {
                            parts.push(json!({"text": s}));
                        }
                    }
                    OpenAIContent::Array(blocks) => {
                        for block in blocks {
                            match block {
                                OpenAIContentBlock::Text { text } => {
                                    parts.push(json!({"text": text}));
                                }
                                OpenAIContentBlock::ImageUrl { image_url } => {
                                    if image_url.url.starts_with("data:") {
                                        if let Some(pos) = image_url.url.find(",") {
                                            let mime_part = &image_url.url[5..pos];
                                            let mime_type = mime_part.split(';').next().unwrap_or("image/jpeg");
                                            let data = &image_url.url[pos + 1..];
                                            
                                            parts.push(json!({
                                                "inlineData": { "mimeType": mime_type, "data": data }
                                            }));
                                        }
                                    } else if image_url.url.starts_with("http") {
                                        parts.push(json!({
                                            "fileData": { "fileUri": &image_url.url, "mimeType": "image/jpeg" }
                                        }));
                                    } else {
                                        // [NEW] 处理本地文件路径 (file:// 或 Windows/Unix 路径)
                                        let file_path = if image_url.url.starts_with("file://") {
                                            // 移除 file:// 前缀
                                            #[cfg(target_os = "windows")]
                                            { image_url.url.trim_start_matches("file:///").replace('/', "\\") }
                                            #[cfg(not(target_os = "windows"))]
                                            { image_url.url.trim_start_matches("file://").to_string() }
                                        } else {
                                            image_url.url.clone()
                                        };
                                        
                                        tracing::debug!("[OpenAI-Request] Reading local image: {}", file_path);
                                        
                                        // 读取文件并转换为 base64
                                        if let Ok(file_bytes) = std::fs::read(&file_path) {
                                            use base64::Engine as _;
                                            let b64 = base64::engine::general_purpose::STANDARD.encode(&file_bytes);
                                            
                                            // 根据文件扩展名推断 MIME 类型
                                            let mime_type = if file_path.to_lowercase().ends_with(".png") {
                                                "image/png"
                                            } else if file_path.to_lowercase().ends_with(".gif") {
                                                "image/gif"
                                            } else if file_path.to_lowercase().ends_with(".webp") {
                                                "image/webp"
                                            } else {
                                                "image/jpeg"
                                            };
                                            
                                            parts.push(json!({
                                                "inlineData": { "mimeType": mime_type, "data": b64 }
                                            }));
                                            tracing::debug!("[OpenAI-Request] Successfully loaded image: {} ({} bytes)", file_path, file_bytes.len());
                                        } else {
                                            tracing::debug!("[OpenAI-Request] Failed to read local image: {}", file_path);
                                        }
                                    }
                                }
                                OpenAIContentBlock::AudioUrl { audio_url: _ } => {
                                    // 暂时跳过 audio_url 处理
                                    // 完整实现需要下载音频文件并转换为 Gemini inlineData 格式
                                    // 这会与 v3.3.16 的 thinkingConfig 逻辑冲突，留待后续版本实现
                                    tracing::debug!("[OpenAI-Request] Skipping audio_url (not yet implemented in v3.3.16)");
                                }
                            }
                        }
                    }
                }
            }

            // Handle tool calls (assistant message)
            if let Some(tool_calls) = &msg.tool_calls {
                for (_index, tc) in tool_calls.iter().enumerate() {
                    /* 暂时移除：防止 Codex CLI 界面碎片化
                    if index == 0 && parts.is_empty() {
                         if mapped_model.contains("gemini-3") {
                              parts.push(json!({"text": "Thinking Process: Determining necessary tool actions."}));
                         }
                    }
                    */


                    let mut args = serde_json::from_str::<Value>(&tc.function.arguments).unwrap_or(json!({}));
                    
                    // [New] 利用通用引擎修正参数类型 (替代以前硬编码的 shell 工具修复逻辑)
                    if let Some(original_schema) = tool_name_to_schema.get(&tc.function.name) {
                        crate::proxy::common::json_schema::fix_tool_call_args(&mut args, original_schema);
                    }

                    let mut func_call_part = json!({
                        "functionCall": {
                            "name": if tc.function.name == "local_shell_call" { "shell" } else { &tc.function.name },
                            "args": args,
                            "id": &tc.id,
                        }
                    });

                    // [New] 递归清理参数中可能存在的非法校验字段
                    crate::proxy::common::json_schema::clean_json_schema(&mut func_call_part);

                    // [修复] 为该消息内的所有工具调用注入 thoughtSignature
                    if let Some(ref sig) = global_thought_sig {
                        func_call_part["thoughtSignature"] = json!(sig);
                    } else if is_thinking_model && !mapped_model.starts_with("projects/") {
                        // [NEW] Handle missing signature for Gemini thinking models
                        tracing::debug!("[OpenAI-Signature] Adding GEMINI_SKIP_SIGNATURE for tool_use: {}", tc.id);
                        func_call_part["thoughtSignature"] = json!("skip_thought_signature_validator");
                    }

                    parts.push(func_call_part);
                }
            }

            // Handle tool response
            if msg.role == "tool" || msg.role == "function" {
                let name = msg.name.as_deref().unwrap_or("unknown");
                let final_name = if name == "local_shell_call" { "shell" } 
                                else if let Some(id) = &msg.tool_call_id { tool_id_to_name.get(id).map(|s| s.as_str()).unwrap_or(name) }
                                else { name };

                let content_val = match &msg.content {
                    Some(OpenAIContent::String(s)) => s.clone(),
                    Some(OpenAIContent::Array(blocks)) => blocks.iter().filter_map(|b| if let OpenAIContentBlock::Text { text } = b { Some(text.clone()) } else { None }).collect::<Vec<_>>().join("\n"),
                    None => "".to_string()
                };

                parts.push(json!({
                    "functionResponse": {
                       "name": final_name,
                       "response": { "result": content_val },
                       "id": msg.tool_call_id.clone().unwrap_or_default()
                    }
                }));
            }

            json!({ "role": role, "parts": parts })
        })
        .filter(|msg| !msg["parts"].as_array().map(|a| a.is_empty()).unwrap_or(true))
        .collect();

    // 合并连续相同角色的消息 (Gemini 强制要求 user/model 交替)
    let mut merged_contents: Vec<Value> = Vec::new();
    for msg in contents {
        if let Some(last) = merged_contents.last_mut() {
            if last["role"] == msg["role"] {
                // 合并 parts
                if let (Some(last_parts), Some(msg_parts)) =
                    (last["parts"].as_array_mut(), msg["parts"].as_array())
                {
                    last_parts.extend(msg_parts.iter().cloned());
                    continue;
                }
            }
        }
        merged_contents.push(msg);
    }
    let contents = merged_contents;

    // 3. 构建请求体

    let mut gen_config = json!({
        "maxOutputTokens": request.max_tokens.unwrap_or(16384),
        "temperature": request.temperature.unwrap_or(1.0),
        "topP": request.top_p.unwrap_or(1.0),
    });

    // [NEW] 支持多候选结果数量 (n -> candidateCount)
    if let Some(n) = request.n {
        gen_config["candidateCount"] = json!(n);
    }

    // 为 thinking 模型注入 thinkingConfig (使用 thinkingBudget 而非 thinkingLevel)
    if actual_include_thinking {
        gen_config["thinkingConfig"] = json!({
            "includeThoughts": true,
            "thinkingBudget": 16000
        });
        tracing::debug!(
            "[OpenAI-Request] Injected thinkingConfig for model {}: thinkingBudget=16000",
            mapped_model
        );
    }

    if let Some(stop) = &request.stop {
        if stop.is_string() {
            gen_config["stopSequences"] = json!([stop]);
        } else if stop.is_array() {
            gen_config["stopSequences"] = stop.clone();
        }
    }

    if let Some(fmt) = &request.response_format {
        if fmt.r#type == "json_object" {
            gen_config["responseMimeType"] = json!("application/json");
        }
    }

    let mut inner_request = json!({
        "contents": contents,
        "generationConfig": gen_config,
        "safetySettings": [
            { "category": "HARM_CATEGORY_HARASSMENT", "threshold": "OFF" },
            { "category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "OFF" },
            { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "OFF" },
            { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "OFF" },
            { "category": "HARM_CATEGORY_CIVIC_INTEGRITY", "threshold": "OFF" },
        ]
    });

    // 深度清理 [undefined] 字符串 (Cherry Studio 等客户端常见注入)
    crate::proxy::mappers::common_utils::deep_clean_undefined(&mut inner_request);

    // 4. Handle Tools (Merged Cleaning)
    if let Some(tools) = &request.tools {
        let mut function_declarations: Vec<Value> = Vec::new();
        for tool in tools.iter() {
            let mut gemini_func = if let Some(func) = tool.get("function") {
                func.clone()
            } else {
                let mut func = tool.clone();
                if let Some(obj) = func.as_object_mut() {
                    obj.remove("type");
                    obj.remove("strict");
                    obj.remove("additionalProperties");
                }
                func
            };

            if let Some(name) = gemini_func.get("name").and_then(|v| v.as_str()) {
                // 跳过内置联网工具名称，避免重复定义
                if name == "web_search" || name == "google_search" || name == "web_search_20250305"
                {
                    continue;
                }

                if name == "local_shell_call" {
                    if let Some(obj) = gemini_func.as_object_mut() {
                        obj.insert("name".to_string(), json!("shell"));
                    }
                }
            }

            // [NEW CRITICAL FIX] 清除函数定义根层级的非法字段 (解决报错持久化)
            if let Some(obj) = gemini_func.as_object_mut() {
                obj.remove("format");
                obj.remove("strict");
                obj.remove("additionalProperties");
                obj.remove("type"); // [NEW] Gemini 不支持在 FunctionDeclaration 根层级出现 type: "function"
            }

            if let Some(params) = gemini_func.get_mut("parameters") {
                // [DEEP FIX] 统一调用公共库清洗：展开 $ref 并剔除所有层级的 format/definitions
                crate::proxy::common::json_schema::clean_json_schema(params);

                // Gemini v1internal 要求：
                // 1. type 必须是大写 (OBJECT, STRING 等)
                // 2. 根对象必须有 "type": "OBJECT"
                if let Some(params_obj) = params.as_object_mut() {
                    if !params_obj.contains_key("type") {
                        params_obj.insert("type".to_string(), json!("OBJECT"));
                    }
                }

                // 递归转换 type 为大写 (符合 Protobuf 定义)
                enforce_uppercase_types(params);
            } else {
                // [FIX] 针对自定义工具 (如 apply_patch) 补全缺失的参数模式
                // 解决 Vertex AI (Claude) 报错: tools.5.custom.input_schema: Field required
                tracing::debug!(
                    "[OpenAI-Request] Injecting default schema for custom tool: {}",
                    gemini_func
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );

                gemini_func.as_object_mut().unwrap().insert(
                    "parameters".to_string(),
                    json!({
                        "type": "OBJECT",
                        "properties": {
                            "content": {
                                "type": "STRING",
                                "description": "The raw content or patch to be applied"
                            }
                        },
                        "required": ["content"]
                    }),
                );
            }
            function_declarations.push(gemini_func);
        }

        if !function_declarations.is_empty() {
            inner_request["tools"] = json!([{ "functionDeclarations": function_declarations }]);
        }
    }

    // [NEW] Antigravity 身份指令 (原始简化版)
    let antigravity_identity = "You are Antigravity, a powerful agentic AI coding assistant designed by the Google Deepmind team working on Advanced Agentic Coding.\n\
    You are pair programming with a USER to solve their coding task. The task may require creating a new codebase, modifying or debugging an existing codebase, or simply answering a question.\n\
    **Absolute paths only**\n\
    **Proactiveness**";

    // [HYBRID] 检查用户是否已提供 Antigravity 身份
    let user_has_antigravity = system_instructions
        .iter()
        .any(|s| s.contains("You are Antigravity"));

    let mut parts = Vec::new();

    // 1. Antigravity 身份 (如果需要, 作为独立 Part 插入)
    if !user_has_antigravity {
        parts.push(json!({"text": antigravity_identity}));
    }

    // 2. 追加用户指令 (作为独立 Parts)
    for inst in system_instructions {
        parts.push(json!({"text": inst}));
    }

    inner_request["systemInstruction"] = json!({
        "role": "user",
        "parts": parts
    });

    if config.inject_google_search {
        crate::proxy::mappers::common_utils::inject_google_search_tool(&mut inner_request);
    }

    if let Some(image_config) = config.image_config {
        if let Some(obj) = inner_request.as_object_mut() {
            obj.remove("tools");
            obj.remove("systemInstruction");
            let gen_config = obj.entry("generationConfig").or_insert_with(|| json!({}));
            if let Some(gen_obj) = gen_config.as_object_mut() {
                gen_obj.remove("thinkingConfig");
                gen_obj.remove("responseMimeType");
                gen_obj.remove("responseModalities");
                gen_obj.insert("imageConfig".to_string(), image_config);
            }
        }
    }

    json!({
        "project": project_id,
        "requestId": format!("openai-{}", uuid::Uuid::new_v4()),
        "request": inner_request,
        "model": config.final_model,
        "userAgent": "antigravity",
        "requestType": config.request_type
    })
}

fn enforce_uppercase_types(value: &mut Value) {
    if let Value::Object(map) = value {
        if let Some(type_val) = map.get_mut("type") {
            if let Value::String(ref mut s) = type_val {
                *s = s.to_uppercase();
            }
        }
        if let Some(properties) = map.get_mut("properties") {
            if let Value::Object(ref mut props) = properties {
                for v in props.values_mut() {
                    enforce_uppercase_types(v);
                }
            }
        }
        if let Some(items) = map.get_mut("items") {
            enforce_uppercase_types(items);
        }
    } else if let Value::Array(arr) = value {
        for item in arr {
            enforce_uppercase_types(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_openai_request_multimodal() {
        let req = OpenAIRequest {
            model: "gpt-4-vision".to_string(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: Some(OpenAIContent::Array(vec![
                    OpenAIContentBlock::Text { text: "What is in this image?".to_string() },
                    OpenAIContentBlock::ImageUrl { image_url: OpenAIImageUrl { 
                        url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==".to_string(),
                        detail: None 
                    } }
                ])),
                reasoning_content: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }],
            stream: false,
            n: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            instructions: None,
            input: None,
            prompt: None,
            size: None,
            quality: None,
            person_generation: None,
        };

        let result = transform_openai_request(&req, "test-v", "gemini-1.5-flash");
        let parts = &result["request"]["contents"][0]["parts"];
        assert_eq!(parts.as_array().unwrap().len(), 2);
        assert_eq!(parts[0]["text"].as_str().unwrap(), "What is in this image?");
        assert_eq!(
            parts[1]["inlineData"]["mimeType"].as_str().unwrap(),
            "image/png"
        );
    }
}
