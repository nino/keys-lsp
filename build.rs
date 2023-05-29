use std::path::PathBuf;

fn main() {
    let grammar_dir: PathBuf = ["tree-sitter-json", "src"].iter().collect();
    cc::Build::new()
        .include(&grammar_dir)
        .file(grammar_dir.join("parser.c"))
        .compile("tree-sitter-json");
}
