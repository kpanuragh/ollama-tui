
use crate::{events::AppEvent, models};
use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use tokio::sync::mpsc;

pub async fn fetch_models(
    client: &Client,
    base_url: &str,
    auth_enabled: bool,
    auth_method: Option<&models::AuthMethod>,
) -> Result<Vec<String>, String> {
    let url = format!("{}/api/tags", base_url);
    let mut request_builder = client.get(&url);

    if auth_enabled {
        if let Some(models::AuthMethod::Basic { username, password }) = auth_method {
            request_builder = request_builder.basic_auth(username, Some(password));
        }
    }

    let response = request_builder.send().await.map_err(|e| e.to_string())?;

    if response.status().is_success() {
        let models_response: models::ModelsResponse = response.json().await.map_err(|e| e.to_string())?;
        Ok(models_response
            .models
            .into_iter()
            .map(|m| m.name)
            .collect())
    } else {
        Err(format!(
            "Failed to fetch models: {}",
            response.status()
        ))
    }
}

pub async fn stream_chat_request(
    client: &Client,
    base_url: &str,
    model: &str,
    messages: &[models::Message],
    auth_enabled: bool,
    auth_method: Option<&models::AuthMethod>,
    tx: mpsc::Sender<AppEvent>,
) {
    let url = format!("{}/api/chat", base_url);
    let request_payload = models::ChatRequest {
        model,
        messages,
        stream: true,
    };

    let mut request_builder = client.post(&url).json(&request_payload);

    if auth_enabled {
        if let Some(models::AuthMethod::Basic { username, password }) = auth_method {
            request_builder = request_builder.basic_auth(username, Some(password));
        }
    }

    let res = match request_builder.send().await {
        Ok(res) => res,
        Err(e) => {
            tx.send(AppEvent::OllamaChunk(Err(e.to_string())))
                .await
                .ok();
            tx.send(AppEvent::OllamaDone).await.ok();
            return;
        }
    };
    if !res.status().is_success() {
        let err_body = res
            .text()
            .await
            .unwrap_or_else(|_| "Unknown API error".to_string());
        tx.send(AppEvent::OllamaChunk(Err(err_body))).await.ok();
        tx.send(AppEvent::OllamaDone).await.ok();
        return;
    }
    let mut stream = res.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                let data = String::from_utf8_lossy(&chunk);
                for line in data.lines() {
                    if line.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<models::StreamChatResponse>(line) {
                        Ok(stream_res) => {
                            tx.send(AppEvent::OllamaChunk(Ok(stream_res.message.content)))
                                .await
                                .ok();
                            if stream_res.done {
                                tx.send(AppEvent::OllamaDone).await.ok();
                                return;
                            }
                        }
                        Err(e) => {
                            let err_msg =
                                format!("Failed to parse stream JSON: {} on line '{}'", e, line);
                            tx.send(AppEvent::OllamaChunk(Err(err_msg))).await.ok();
                        }
                    }
                }
            }
            Err(e) => {
                tx.send(AppEvent::OllamaChunk(Err(e.to_string())))
                    .await
                    .ok();
                break;
            }
        }
    }
    tx.send(AppEvent::OllamaDone).await.ok();
}

