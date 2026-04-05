pub mod code_map;
pub mod lsp;
use std::path::Path;
use tree_sitter::Parser;

pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub line: usize,
}

pub struct SymbolExtractor {
    parser: Parser,
}

impl SymbolExtractor {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
        }
    }

    pub fn extract(&mut self, path: &Path, content: &str) -> Vec<Symbol> {
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let language = match extension {
            "rs" => Some(tree_sitter_rust::language().into()),
            "js" => Some(tree_sitter_javascript::language().into()),
            "ts" | "tsx" => Some(tree_sitter_typescript::language_typescript().into()),
            "py" => Some(tree_sitter_python::language().into()),
            _ => None,
        };

        let lang = match language {
            Some(l) => l,
            None => return vec![],
        };
        self.parser.set_language(&lang).unwrap();

        let tree = self.parser.parse(content, None).unwrap();
        let mut symbols = Vec::new();

        self.traverse(tree.root_node(), content, &mut symbols);
        symbols
    }

    fn traverse(&self, node: tree_sitter::Node, content: &str, symbols: &mut Vec<Symbol>) {
        let kind = node.kind();

        match kind {
            "function_item" | "method_declaration" | "function_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = &content[name_node.start_byte()..name_node.end_byte()];
                    symbols.push(Symbol {
                        name: name.to_string(),
                        kind: "function".to_string(),
                        line: node.start_position().row + 1,
                    });
                }
            }
            "struct_item" | "class_declaration" | "class_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = &content[name_node.start_byte()..name_node.end_byte()];
                    symbols.push(Symbol {
                        name: name.to_string(),
                        kind: "class".to_string(),
                        line: node.start_position().row + 1,
                    });
                }
            }
            _ => {}
        }

        for i in 0..node.child_count() {
            self.traverse(node.child(i).unwrap(), content, symbols);
        }
    }
}
