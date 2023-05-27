use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
mod logger;

#[derive(Debug)]
struct Config {
    files: HashMap<String, PathBuf>,
}

impl Config {
    fn init() -> Self {
        let mut files = HashMap::new();
        let files_config = std::env::var("KEYS_LSP_FILES").unwrap_or("".to_string());
        let entries = files_config.split(',');
        for entry in entries {
            let parts: Vec<&str> = entry.split(':').collect();
            if parts.len() != 2 {
                continue;
            }
            let prefix = parts[0];
            let path = parts[1];
            files.insert(prefix.to_string(), PathBuf::from(path));
        }
        Self { files }
    }
}

fn get_hovered_line(params: &HoverParams) -> Result<String> {
    let file = std::fs::File::open(
        params
            .text_document_position_params
            .text_document
            .uri
            .path(),
    )?;
    let line_number = params.text_document_position_params.position.line;
    let mut lines = std::io::BufReader::new(file).lines();
    let line = lines
        .nth(line_number as usize)
        .ok_or(anyhow!("Line doesn't exist"))??;

    Ok(line)
}

fn get_string_around_cursor(line: &str, cursor: usize) -> Option<String> {
    if line.len() <= cursor {
        return None;
    }
    let left = &line[0..cursor];
    let string_start = left.rfind('"')? + 1;
    let right = &line[cursor..];
    let string_end = cursor + right.find('"')?;
    Some(line[string_start..string_end].to_string())
}

fn get_hovered_string(params: &HoverParams) -> Option<String> {
    let line = get_hovered_line(&params).ok()?;
    get_string_around_cursor(
        &line,
        params.text_document_position_params.position.character as usize,
    )
}

#[derive(Debug)]
struct Backend {
    client: Client,
    config: Config,
}

impl Backend {
    fn get_value(&self, key: &str) -> Option<String> {
        let key_parts: Vec<String> = key
            .split(|c| c == ':' || c == '.')
            .map(|s| s.into())
            .collect();
        logger::log(&format!("Getting value for key: {:?}", key_parts));
        let prefix = key_parts.get(0)?;
        let requested_file = self.config.files.get(prefix)?;
        let file = std::fs::File::open(requested_file).ok()?;
        let reader = std::io::BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader).ok()?;
        let mut value = &json;
        logger::log(&format!("Found json: {:?}", json));
        let mut next_part = key_parts.iter().skip(1).next()?;
        while let Some(nested_value) = value.get(next_part) {
            logger::log(&format!("Found nested value: {:?}", nested_value));
            value = nested_value;
            next_part = key_parts.iter().next()?;
        }
        match value {
            serde_json::Value::String(s) => return Some(s.to_string()),
            serde_json::Value::Object(obj) => return Some(serde_json::to_string(obj).ok()?),
            _ => return None,
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        let mut res = InitializeResult::default();
        res.capabilities.hover_provider = Some(HoverProviderCapability::Simple(true));
        Ok(res)
    }

    async fn initialized(&self, _: InitializedParams) {
        logger::log("server initialized!");
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn did_change(&self, _: DidChangeTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file changed!")
            .await;
    }

    async fn hover(&self, params: HoverParams) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let hovered_string = get_hovered_string(&params);
        match hovered_string {
            Some(s) => match self.get_value(&s) {
                Some(v) => Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(v)),
                    range: None,
                })),
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }
}

// "compstool:abc"
// "compstool:nested.works"
// "compstool:nested.works.${things}"

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let config = Config::init();

    let (service, socket) = LspService::new(|client| Backend { client, config });
    Server::new(stdin, stdout, socket).serve(service).await;
}
