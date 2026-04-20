use oxc::ast::ast::{TSType, TSTypeAliasDeclaration};
use oxc::span::Span;

use crate::shared_lib::{
    byte_offset_to_line_col, AstNodeVariant, DeclarationChecker, FoundDeclarationNode,
};

pub struct TypeChecker<'a> {
    pub type_alias: &'a TSTypeAliasDeclaration<'a>,
}

impl<'a> DeclarationChecker for TypeChecker<'a> {
    fn from_ast(
        &self,
        source: &str,
        filename: &str,
        is_exported: bool,
        override_span: Option<Span>,
    ) -> FoundDeclarationNode {
        let name = self.type_alias.id.name.to_string();

        let span = override_span.unwrap_or(self.type_alias.span);
        let start = span.start as usize;
        let end = span.end as usize;

        let (line, col) = byte_offset_to_line_col(source, start);
        let body = serialize_ts_type(&self.type_alias.type_annotation);

        FoundDeclarationNode {
            ast_node_variant: AstNodeVariant::Type,
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

pub fn serialize_ts_type(ts_type: &TSType) -> String {
    match ts_type {
        TSType::TSAnyKeyword(_) => "any".to_string(),
        TSType::TSBooleanKeyword(_) => "boolean".to_string(),
        TSType::TSNumberKeyword(_) => "number".to_string(),
        TSType::TSStringKeyword(_) => "string".to_string(),
        TSType::TSNullKeyword(_) => "null".to_string(),
        TSType::TSUndefinedKeyword(_) => "undefined".to_string(),
        TSType::TSVoidKeyword(_) => "void".to_string(),
        TSType::TSNeverKeyword(_) => "never".to_string(),
        TSType::TSUnknownKeyword(_) => "unknown".to_string(),
        TSType::TSBigIntKeyword(_) => "bigint".to_string(),
        TSType::TSSymbolKeyword(_) => "symbol".to_string(),
        TSType::TSObjectKeyword(_) => "object".to_string(),
        TSType::TSIntrinsicKeyword(_) => "intrinsic".to_string(),
        TSType::TSThisType(_) => "this".to_string(),

        TSType::TSTypeReference(r) => {
            let name = format!("{:?}", r.type_name);
            if let Some(params) = &r.type_arguments {
                let ps: Vec<String> = params.params.iter().map(|p| serialize_ts_type(p)).collect();
                format!("{}<{}>", name, ps.join(", "))
            } else {
                name
            }
        }

        TSType::TSTypeLiteral(lit) => {
            let members: Vec<String> = lit.members.iter().map(|m| format!("{:?}", m)).collect();
            format!("{{ {} }}", members.join("; "))
        }

        TSType::TSUnionType(u) => {
            let types: Vec<String> = u.types.iter().map(|t| serialize_ts_type(t)).collect();
            types.join(" | ")
        }

        TSType::TSIntersectionType(i) => {
            let types: Vec<String> = i.types.iter().map(|t| serialize_ts_type(t)).collect();
            types.join(" & ")
        }

        TSType::TSArrayType(a) => format!("{}[]", serialize_ts_type(&a.element_type)),

        TSType::TSTupleType(t) => {
            let elems: Vec<String> = t.element_types.iter().map(|e| format!("{:?}", e)).collect();
            format!("[{}]", elems.join(", "))
        }

        TSType::TSFunctionType(f) => format!("FunctionType({:?})", f.params),
        TSType::TSConstructorType(c) => format!("ConstructorType({:?})", c.params),
        TSType::TSConditionalType(c) => {
            format!(
                "{} extends {} ? {} : {}",
                serialize_ts_type(&c.check_type),
                serialize_ts_type(&c.extends_type),
                serialize_ts_type(&c.true_type),
                serialize_ts_type(&c.false_type)
            )
        }
        TSType::TSTypeQuery(q) => format!("typeof {:?}", q.expr_name),
        TSType::TSIndexedAccessType(i) => {
            format!(
                "{}[{}]",
                serialize_ts_type(&i.object_type),
                serialize_ts_type(&i.index_type)
            )
        }
        TSType::TSMappedType(m) => format!("MappedType({:?})", m.key),
        TSType::TSTypeOperatorType(o) => {
            format!("{:?} {}", o.operator, serialize_ts_type(&o.type_annotation))
        }
        TSType::TSImportType(i) => format!("import({:?})", i.source),
        TSType::TSParenthesizedType(p) => {
            format!("({})", serialize_ts_type(&p.type_annotation))
        }
        TSType::TSInferType(i) => format!("infer {:?}", i.type_parameter),
        TSType::TSLiteralType(l) => format!("{:?}", l.literal),
        TSType::TSTemplateLiteralType(t) => format!("TemplateLiteral({:?})", t.quasis),
        TSType::TSNamedTupleMember(m) => {
            format!("{}: {:?}", m.label, m.element_type)
        }
        TSType::JSDocNullableType(n) => format!("?{}", serialize_ts_type(&n.type_annotation)),
        TSType::JSDocNonNullableType(n) => format!("!{}", serialize_ts_type(&n.type_annotation)),
        TSType::JSDocUnknownType(_) => "unknown(jsdoc)".to_string(),
        TSType::TSTypePredicate(p) => format!("TypePredicate({:?})", p.parameter_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_lib::AstNodeVariant;
    use oxc::allocator::Allocator;
    use oxc::ast::ast::{Declaration, Statement};
    use oxc::parser::Parser as OxcParser;
    use oxc::span::SourceType;

    fn parse_type(source: &str) -> FoundDeclarationNode {
        let allocator = Allocator::default();
        let source_type = SourceType::ts();
        let ret = OxcParser::new(&allocator, source, source_type).parse();
        for stmt in &ret.program.body {
            if let Statement::TSTypeAliasDeclaration(type_alias) = stmt {
                let checker = TypeChecker { type_alias };
                return checker.from_ast(source, "test.ts", false, None);
            }
            if let Statement::ExportNamedDeclaration(export) = stmt {
                if let Some(Declaration::TSTypeAliasDeclaration(type_alias)) = &export.declaration {
                    let checker = TypeChecker { type_alias };
                    return checker.from_ast(source, "test.ts", true, Some(export.span));
                }
            }
        }
        panic!("No type alias found in source");
    }

    #[test]
    fn test_type_checker_name_and_variant() {
        let node = parse_type("type Foo = string;");
        assert_eq!(node.name, "Foo");
        assert!(matches!(node.ast_node_variant, AstNodeVariant::Type));
    }

    #[test]
    fn test_type_checker_body_simple() {
        let node = parse_type("type Foo = string;");
        assert_eq!(node.body, "string");
    }

    #[test]
    fn test_type_checker_body_union() {
        let node = parse_type("type Foo = string | number;");
        assert_eq!(node.body, "string | number");
    }

    #[test]
    fn test_type_checker_not_exported() {
        let node = parse_type("type Foo = string;");
        assert!(!node.is_exported);
    }

    #[test]
    fn test_type_checker_exported() {
        let node = parse_type("export type Foo = string;");
        assert!(node.is_exported);
    }

    #[test]
    fn test_type_checker_identical_bodies_match() {
        let a = parse_type("type Foo = string;");
        let b = parse_type("type Foo = string;");
        assert_eq!(a.body, b.body);
    }

    #[test]
    fn test_type_checker_different_bodies_differ() {
        let a = parse_type("type Foo = string;");
        let b = parse_type("type Foo = number;");
        assert_ne!(a.body, b.body);
    }

    #[test]
    fn test_type_checker_line_col() {
        let node = parse_type("type Foo = string;");
        assert_eq!(node.line, 1);
        assert_eq!(node.col, 1);
    }

    #[test]
    fn test_type_checker_line_col_with_offset() {
        let source = "\n\ntype Foo = string;";
        let node = parse_type(source);
        assert_eq!(node.line, 3);
        assert_eq!(node.col, 1);
    }

    #[test]
    fn test_type_checker_filename() {
        let node = parse_type("type Foo = string;");
        assert_eq!(node.filename, "test.ts");
    }
}
