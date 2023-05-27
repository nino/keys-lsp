use anyhow::{anyhow, Result};
use std::io::BufRead;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
mod logger;

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

// TODO there is an off-by-one-or-even-a-few error in here
fn get_string_around_cursor(line: &str, cursor: usize) -> Result<String> {
    let left = line
        .get(0..cursor)
        .ok_or(anyhow!("Cursor is out of bounds"))?;
    let string_start = left
        .rfind(|c: char| c != '"')
        .ok_or(anyhow!("No quote mark found left of the cursor"))?;
    let right = line
        .get(cursor..)
        .ok_or(anyhow!("Cursor is out of bounds"))?;
    let string_end = cursor
        + right
            .find('"')
            .ok_or(anyhow!("No quote mark found right of the cursor"))?;
    Ok(line
        .get(string_start..string_end)
        .ok_or(anyhow!("String is out of bounds"))?
        .to_string())
}

fn get_hovered_string(params: &HoverParams) -> Result<String> {
    let line = get_hovered_line(&params)?;
    get_string_around_cursor(
        &line,
        params.text_document_position_params.position.character as usize,
    )
}

#[derive(Debug)]
struct Backend {
    client: Client,
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
        let _ = logger::log("server initialized!");
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
        let _ = logger::log(&format!("Hovering! {:?}", params));
        let hovered_string = get_hovered_string(&params).ok();
        self.client
            .log_message(MessageType::INFO, "Hovering!")
            .await;
        match hovered_string {
            Some(s) => Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(s)),
                range: None,
            })),
            None => Ok(None),
        }
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
