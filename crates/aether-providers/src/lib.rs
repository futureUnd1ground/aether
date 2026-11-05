use aether_core::{FunctionCall, Message, ToolCall, Usage, config::ProviderConfig};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub enum Delta {
    Text(String),
    Usage(Usage),
}
#[derive(Debug, Default)]
pub struct Completion {
    pub text: String,
    pub tools: Vec<ToolCall>,
    pub usage: Usage,
}
#[async_trait]
pub trait Provider: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[Value],
        max_tokens: usize,
        events: mpsc::Sender<Delta>,
        cancel: CancellationToken,
    ) -> Result<Completion>;
}
pub struct OpenAiCompatible {
    client: reqwest::Client,
    config: ProviderConfig,
}
impl OpenAiCompatible {
    pub fn new(config: ProviderConfig) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(config.timeout_secs))
                .build()?,
            config,
        })
    }
    fn request(&self, path: &str) -> reqwest::RequestBuilder {
        let r = self.client.request(
            if path == "models" {
                reqwest::Method::GET
            } else {
                reqwest::Method::POST
            },
            format!("{}/{}", self.config.base_url.trim_end_matches('/'), path),
        );
        if let Ok(key) = std::env::var(&self.config.api_key_env) {
            r.bearer_auth(key)
        } else {
            r
        }
    }
    pub async fn models(&self) -> Result<Vec<String>> {
        let r = self.request("models").send().await?;
        if !r.status().is_success() {
            bail!("model discovery returned HTTP {}", r.status());
        }
        let v: Value = r.json().await?;
        Ok(v["data"]
            .as_array()
            .context("missing model list")?
            .iter()
            .filter_map(|m| m["id"].as_str().map(str::to_owned))
            .collect())
    }
}
#[async_trait]
impl Provider for OpenAiCompatible {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[Value],
        max_tokens: usize,
        events: mpsc::Sender<Delta>,
        cancel: CancellationToken,
    ) -> Result<Completion> {
        let mut body = json!({"model":self.config.model,"messages":messages,"stream":true,"max_tokens":max_tokens,"stream_options":{"include_usage":true}});
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        let response = tokio::select! {biased; _=cancel.cancelled()=>bail!("cancelled"),r=self.request("chat/completions").json(&body).send()=>r.context("provider connection failed")?};
        if !response.status().is_success() {
            bail!(
                "provider HTTP {} (check endpoint, credentials and model; request was not retried)",
                response.status()
            );
        }
        if !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|s| s.starts_with("text/event-stream"))
        {
            bail!("provider did not return text/event-stream");
        }
        let mut received = 0usize;
        let bounded = response.bytes_stream().map(move |chunk| {
            let chunk = chunk.map_err(std::io::Error::other)?;
            received = received.saturating_add(chunk.len());
            if received > 8 * 1024 * 1024 {
                return Err(std::io::Error::other("provider response exceeds 8 MiB"));
            }
            Ok(chunk)
        });
        let mut stream = bounded.eventsource();
        let mut result = Completion::default();
        let mut calls = BTreeMap::<usize, ToolCall>::new();
        let mut finished = false;
        let mut size = 0usize;
        loop {
            let next = tokio::select! {biased; _=cancel.cancelled()=>bail!("cancelled"),n=stream.next()=>n};
            let Some(event) = next else { break };
            let event = event.context("invalid or interrupted SSE stream")?;
            size += event.data.len();
            if size > 8 * 1024 * 1024 {
                bail!("provider response exceeds 8 MiB limit");
            }
            if event.data == "[DONE]" {
                finished = true;
                break;
            }
            let v: Value = serde_json::from_str(&event.data).context("invalid JSON in SSE data")?;
            if v.get("error").is_some() {
                bail!("provider reported an error in stream");
            }
            if let Some(u) = v.get("usage").filter(|v| v.is_object()) {
                result.usage = Usage {
                    prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
                    completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
                };
                send(&events, Delta::Usage(result.usage.clone()), &cancel).await?;
            }
            if let Some(choice) = v["choices"].as_array().and_then(|a| a.first()) {
                if let Some(reason) = choice["finish_reason"].as_str() {
                    if !matches!(reason, "stop" | "tool_calls") {
                        bail!("provider stopped with {reason}; incomplete response");
                    }
                    finished = true;
                }
                let d = &choice["delta"];
                if let Some(text) = d["content"].as_str() {
                    result.text.push_str(text);
                    send(&events, Delta::Text(text.into()), &cancel).await?;
                }
                if let Some(ts) = d["tool_calls"].as_array() {
                    for t in ts {
                        let index =
                            t["index"].as_u64().context("tool call missing index")? as usize;
                        if index > 63 {
                            bail!("too many tool calls");
                        }
                        let c = calls.entry(index).or_insert_with(|| ToolCall {
                            id: String::new(),
                            kind: "function".into(),
                            function: FunctionCall {
                                name: String::new(),
                                arguments: String::new(),
                            },
                        });
                        if let Some(s) = t["id"].as_str() {
                            c.id.push_str(s);
                        }
                        if let Some(s) = t["function"]["name"].as_str() {
                            c.function.name.push_str(s);
                        }
                        if let Some(s) = t["function"]["arguments"].as_str() {
                            c.function.arguments.push_str(s);
                        }
                    }
                }
            }
        }
        if !finished {
            bail!("provider stream ended before a completion marker");
        }
        result.tools = calls.into_values().collect();
        let mut ids = std::collections::HashSet::new();
        for c in &result.tools {
            if c.id.is_empty() || c.function.name.is_empty() || !ids.insert(&c.id) {
                bail!("invalid or duplicate tool call ID/name");
            }
            let _: Value =
                serde_json::from_str(&c.function.arguments).context("malformed tool arguments")?;
        }
        Ok(result)
    }
}
async fn send(tx: &mpsc::Sender<Delta>, d: Delta, cancel: &CancellationToken) -> Result<()> {
    tokio::select! {_ =cancel.cancelled()=>bail!("cancelled"),r=tx.send(d)=>{r.context("consumer closed")?;Ok(())}}
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    async fn endpoint(
        body: String,
        status: &str,
    ) -> (OpenAiCompatible, tokio::task::JoinHandle<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            let mut buffer = [0u8; 4096];
            let (boundary, length) = loop {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                data.extend_from_slice(&buffer[..n]);
                if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let h = String::from_utf8_lossy(&data[..i]);
                    let len = h
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(|s| s.parse::<usize>().unwrap())
                        })
                        .unwrap();
                    break (i + 4, len);
                }
            };
            while data.len() < boundary + length {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                data.extend_from_slice(&buffer[..n]);
            }
            let request = serde_json::from_slice(&data[boundary..boundary + length]).unwrap();
            socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
            // Deliberately fragment UTF-8 and SSE framing at arbitrary byte boundaries.
            for b in body.as_bytes().chunks(3) {
                if socket.write_all(b).await.is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
            request
        });
        let p = OpenAiCompatible::new(ProviderConfig {
            base_url: format!("http://{address}/v1"),
            model: "fixture".into(),
            ..Default::default()
        })
        .unwrap();
        (p, task)
    }
    fn event(v: Value) -> String {
        format!("data: {v}\r\n\r\n")
    }
    async fn complete(p: &OpenAiCompatible) -> Result<(Completion, Vec<Delta>)> {
        let (tx, mut rx) = mpsc::channel(32);
        let c = p
            .complete(
                &[Message::text("user", "test")],
                &[],
                128,
                tx,
                CancellationToken::new(),
            )
            .await?;
        let mut ds = vec![];
        while let Some(d) = rx.recv().await {
            ds.push(d);
        }
        Ok((c, ds))
    }
    #[tokio::test]
    async fn fragmented_utf8_tools_and_usage() {
        let body = event(
            json!({"choices":[{"delta":{"content":"Привет 🦀","tool_calls":[{"index":0,"id":"c1","function":{"name":"read_","arguments":"{\"pa"}}]}}]}),
        ) + &event(
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"file","arguments":"th\":\"x\"}"}}]},"finish_reason":"tool_calls"}]}),
        ) + &event(
            json!({"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":9}}),
        ) + "data: [DONE]\r\n\r\n";
        let (p, task) = endpoint(body, "200 OK").await;
        let (c, d) = complete(&p).await.unwrap();
        assert_eq!(c.text, "Привет 🦀");
        assert_eq!(c.tools[0].function.name, "read_file");
        assert_eq!(c.tools[0].function.arguments, "{\"path\":\"x\"}");
        assert_eq!(c.usage.completion_tokens, 9);
        assert_eq!(d.len(), 2);
        let r = task.await.unwrap();
        assert_eq!(r["stream"], true);
    }
    #[tokio::test]
    async fn broken_stream_is_error() {
        let (p, t) = endpoint(
            event(json!({"choices":[{"delta":{"content":"partial"}}]})),
            "200 OK",
        )
        .await;
        assert!(
            complete(&p)
                .await
                .unwrap_err()
                .to_string()
                .contains("completion marker")
        );
        t.await.unwrap();
    }
    #[tokio::test]
    async fn malformed_json_and_rate_limit_are_errors() {
        for (body, status) in [
            ("data: nope\n\n".into(), "200 OK"),
            ("rate limited".into(), "429 Too Many Requests"),
        ] {
            let (p, t) = endpoint(body, status).await;
            assert!(complete(&p).await.is_err());
            t.await.unwrap();
        }
    }
    #[tokio::test]
    async fn cancellation_interrupts_waiting_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let p = OpenAiCompatible::new(ProviderConfig {
            base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
            ..Default::default()
        })
        .unwrap();
        let c = CancellationToken::new();
        let token = c.clone();
        let (tx, _) = mpsc::channel(1);
        let work = tokio::spawn(async move { p.complete(&[], &[], 100, tx, token).await });
        tokio::task::yield_now().await;
        c.cancel();
        assert!(
            tokio::time::timeout(Duration::from_secs(1), work)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
    }
}
