use oxc::span::Span;

#[derive(Debug, Clone)]
pub enum AstNodeVariant {
    Type,
    Interface,
}

#[derive(Debug, Clone)]
pub struct FoundDeclarationNode {
    pub ast_node_variant: AstNodeVariant,
    pub name: String,
    pub filename: String,
    pub line: usize,
    pub col: usize,
    pub span_start: usize,
    pub span_end: usize,
    pub is_exported: bool,
    pub body: String,
}

pub trait DeclarationChecker {
    fn from_ast(
        &self,
        source: &str,
        filename: &str,
        is_exported: bool,
        override_span: Option<Span>,
    ) -> FoundDeclarationNode;
}

pub fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
