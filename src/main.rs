use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, TreeCursor};
mod logger;

extern "C" {
    fn tree_sitter_json() -> Language;
}

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

    async fn goto_definition(
        &self,
        _: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        Ok(None)
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

fn walk_the_tree_i_guess(source: &[u8], cursor: &mut TreeCursor) {
    println!(
        "Cursor node: {:?}, {:?}",
        cursor.node(),
        cursor.node().utf8_text(source)
    );
    if cursor.goto_first_child() {
        walk_the_tree_i_guess(source, cursor);
    }
    if cursor.goto_next_sibling() {
        walk_the_tree_i_guess(source, cursor);
    }
    cursor.goto_parent();
}

#[tokio::main]
async fn main() {
    // Trying to understand tree-sitter
    let mut parser = Parser::new();
    let language = unsafe { tree_sitter_json() };
    parser.set_language(language).unwrap();
    let json_content = r#"{ "abc": "def", "nested": { "works": "yes" } }"#;
    let tree = parser.parse(json_content, None).unwrap();
    let root_node = tree.root_node();
    let mut cursor = root_node.walk();
    walk_the_tree_i_guess(json_content.as_bytes(), &mut cursor);
    // End of tree-sitter stuff

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let config = Config::init();

    let (service, socket) = LspService::new(|client| Backend { client, config });
    Server::new(stdin, stdout, socket).serve(service).await;
}
