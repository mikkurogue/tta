use oxc::ast::ast::{PropertyKey, TSInterfaceDeclaration, TSSignature};
use oxc::span::Span;

use crate::shared_lib::{
    byte_offset_to_line_col, AstNodeVariant, DeclarationChecker, FoundDeclarationNode,
};
use crate::type_checker::serialize_ts_type;

pub struct InterfaceChecker<'a> {
    pub interface_decl: &'a TSInterfaceDeclaration<'a>,
}

impl<'a> DeclarationChecker for InterfaceChecker<'a> {
    fn from_ast(
        &self,
        source: &str,
        filename: &str,
        is_exported: bool,
        override_span: Option<Span>,
    ) -> FoundDeclarationNode {
        let name = self.interface_decl.id.name.to_string();

        let span = override_span.unwrap_or(self.interface_decl.span);
        let start = span.start as usize;
        let end = span.end as usize;

        let (line, col) = byte_offset_to_line_col(source, start);

        let body = serialize_interface_body(&self.interface_decl.body.body);

        FoundDeclarationNode {
            ast_node_variant: AstNodeVariant::Interface,
            name,
            body,
            filename: filename.to_string(),
            line,
            col,
            span_start: start,
            span_end: end,
            is_exported,
        }
    }
}

fn serialize_property_key(key: &PropertyKey) -> String {
    match key {
        PropertyKey::StaticIdentifier(id) => id.name.to_string(),
        PropertyKey::PrivateIdentifier(id) => format!("#{}", id.name),
        _ => format!("[{:?}]", key),
    }
}

fn serialize_interface_body(members: &[TSSignature]) -> String {
    let parts: Vec<String> = members
        .iter()
        .map(|sig| match sig {
            TSSignature::TSPropertySignature(prop) => {
                let key = serialize_property_key(&prop.key);
                let opt = if prop.optional { "?" } else { "" };
                let readonly = if prop.readonly { "readonly " } else { "" };
                let ty = prop
                    .type_annotation
                    .as_ref()
                    .map(|ta| serialize_ts_type(&ta.type_annotation))
                    .unwrap_or_else(|| "unknown".to_string());
                format!("{}{}{}: {}", readonly, key, opt, ty)
            }
            TSSignature::TSMethodSignature(method) => {
                let key = serialize_property_key(&method.key);
                let opt = if method.optional { "?" } else { "" };
                let ret = method
                    .return_type
                    .as_ref()
                    .map(|ta| serialize_ts_type(&ta.type_annotation))
                    .unwrap_or_else(|| "void".to_string());
                format!("{}{}(): {}", key, opt, ret)
            }
            TSSignature::TSIndexSignature(idx) => {
                let params: Vec<String> = idx
                    .parameters
                    .iter()
                    .map(|p| {
                        let ty = serialize_ts_type(&p.type_annotation.type_annotation);
                        format!("{}: {}", p.name, ty)
                    })
                    .collect();
                let ty = serialize_ts_type(&idx.type_annotation.type_annotation);
                let readonly = if idx.readonly { "readonly " } else { "" };
                format!("{}[{}]: {}", readonly, params.join(", "), ty)
            }
            TSSignature::TSCallSignatureDeclaration(call) => {
                let ret = call
                    .return_type
                    .as_ref()
                    .map(|ta| serialize_ts_type(&ta.type_annotation))
                    .unwrap_or_else(|| "void".to_string());
                format!("(): {}", ret)
            }
            TSSignature::TSConstructSignatureDeclaration(ctor) => {
                let ret = ctor
                    .return_type
                    .as_ref()
                    .map(|ta| serialize_ts_type(&ta.type_annotation))
                    .unwrap_or_else(|| "void".to_string());
                format!("new(): {}", ret)
            }
        })
        .collect();
    format!("{{ {} }}", parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_lib::AstNodeVariant;
    use oxc::allocator::Allocator;
    use oxc::ast::ast::{Declaration, Statement};
    use oxc::parser::Parser as OxcParser;
    use oxc::span::SourceType;

    fn parse_interface(source: &str) -> FoundDeclarationNode {
        let allocator = Allocator::default();
        let source_type = SourceType::ts();
        let ret = OxcParser::new(&allocator, source, source_type).parse();
        for stmt in &ret.program.body {
            if let Statement::TSInterfaceDeclaration(iface) = stmt {
                let checker = InterfaceChecker {
                    interface_decl: iface,
                };
                return checker.from_ast(source, "test.ts", false, None);
            }
            if let Statement::ExportNamedDeclaration(export) = stmt {
                if let Some(Declaration::TSInterfaceDeclaration(iface)) = &export.declaration {
                    let checker = InterfaceChecker {
                        interface_decl: iface,
                    };
                    return checker.from_ast(source, "test.ts", true, Some(export.span));
                }
            }
        }
        panic!("No interface found in source");
    }

    #[test]
    fn test_interface_checker_name_and_variant() {
        let node = parse_interface("interface Foo { x: string; }");
        assert_eq!(node.name, "Foo");
        assert!(matches!(node.ast_node_variant, AstNodeVariant::Interface));
    }

    #[test]
    fn test_interface_checker_body_properties() {
        let node = parse_interface("interface Foo { name: string; age: number; }");
        assert_eq!(node.body, "{ name: string; age: number }");
    }

    #[test]
    fn test_interface_checker_not_exported() {
        let node = parse_interface("interface Foo { x: string; }");
        assert!(!node.is_exported);
    }

    #[test]
    fn test_interface_checker_exported() {
        let node = parse_interface("export interface Foo { x: string; }");
        assert!(node.is_exported);
    }

    #[test]
    fn test_interface_checker_identical_bodies_match() {
        let a = parse_interface("interface Foo { x: string; y: number; }");
        let b = parse_interface("interface Bar { x: string; y: number; }");
        assert_eq!(a.body, b.body);
    }

    #[test]
    fn test_interface_checker_different_bodies_differ() {
        let a = parse_interface("interface Foo { x: string; }");
        let b = parse_interface("interface Foo { x: number; }");
        assert_ne!(a.body, b.body);
    }

    #[test]
    fn test_interface_checker_optional_property() {
        let node = parse_interface("interface Foo { x?: string; }");
        assert_eq!(node.body, "{ x?: string }");
    }

    #[test]
    fn test_interface_checker_readonly_property() {
        let node = parse_interface("interface Foo { readonly x: string; }");
        assert_eq!(node.body, "{ readonly x: string }");
    }

    #[test]
    fn test_interface_checker_method_signature() {
        let node = parse_interface("interface Foo { greet(): string; }");
        assert_eq!(node.body, "{ greet(): string }");
    }

    #[test]
    fn test_interface_checker_empty_body() {
        let node = parse_interface("interface Foo {}");
        assert_eq!(node.body, "{  }");
    }

    #[test]
    fn test_interface_checker_filename() {
        let node = parse_interface("interface Foo { x: string; }");
        assert_eq!(node.filename, "test.ts");
    }
}
