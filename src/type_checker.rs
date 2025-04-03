use swc_common::{sync::Lrc, SourceMap};
use swc_ecma_ast::{TsType, TsTypeAliasDecl};

#[derive(Debug, Clone)]
pub struct FoundType {
    pub name: String,
    pub filename: String,
    pub line: usize,
    pub is_exported: bool,
    pub body: String,
}

impl FoundType {
    pub fn from_ast(
        type_alias: &TsTypeAliasDecl,
        cm: &Lrc<SourceMap>,
        fm: &Lrc<swc_common::SourceFile>,
        filename: &str,
    ) -> Self {
        let name = type_alias.id.sym.to_string();
        let line = cm
            .lookup_line(fm.start_pos + type_alias.span.lo)
            .map(|pos| pos.line + 1)
            .unwrap_or(0);

        let is_exported = matches!(type_alias.declare, true);

        let body = serialize_ts_type(&type_alias.type_ann);

        Self {
            name,
            body,
            filename: filename.to_string(),
            line,
            is_exported,
        }
    }
}

/// Serialize a TsType from swc to a string
fn serialize_ts_type(ts_type: &TsType) -> String {
    match ts_type {
        TsType::TsKeywordType(keyword) => format!("KeywordType({:?})", keyword.kind),
        TsType::TsTypeRef(type_ref) => {
            // Handle TsEntityName (it can be TsQualifiedName or TsEntityName)
            let type_name = match &type_ref.type_name {
                swc_ecma_ast::TsEntityName::TsQualifiedName(qualified_name) => {
                    format!("QualifiedName({:?})", qualified_name)
                }
                swc_ecma_ast::TsEntityName::Ident(ident) => {
                    format!("Ident({})", ident.sym)
                }
            };
            format!("TypeRef({}, {:?})", type_name, type_ref.type_params)
        }
        TsType::TsTypeLit(type_lit) => format!("TypeLit({:?})", type_lit.members),
        TsType::TsUnionOrIntersectionType(union_or_intersection) => {
            // Handle union or intersection types
            match union_or_intersection {
                swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(union_type) => {
                    format!("UnionType({:?})", union_type.types)
                }
                swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(intersection_type) => {
                    format!("IntersectionType({:?})", intersection_type.types)
                }
            }
        }
        TsType::TsArrayType(array_type) => format!("ArrayType({:?})", array_type.elem_type),
        TsType::TsTupleType(tuple_type) => format!("TupleType({:?})", tuple_type.elem_types),
        TsType::TsFnOrConstructorType(fn_or_constructor) => {
            format!("FnOrConstructorType({:?})", fn_or_constructor)
        }
        TsType::TsConditionalType(conditional_type) => {
            format!("ConditionalType({:?})", conditional_type)
        }
        TsType::TsTypeQuery(type_query) => {
            // Handle TsTypeQuery
            match &type_query.expr_name {
                swc_ecma_ast::TsTypeQueryExpr::TsEntityName(entity_name) => {
                    format!("TypeQuery(TsEntityName({:?}))", entity_name)
                }
                swc_ecma_ast::TsTypeQueryExpr::Import(import) => {
                    format!("TypeQuery(Import({:?}))", import)
                }
            }
        }
        TsType::TsIndexedAccessType(indexed_access) => {
            format!("IndexedAccessType({:?})", indexed_access)
        }
        TsType::TsMappedType(mapped_type) => format!("MappedType({:?})", mapped_type),
        TsType::TsTypeOperator(type_operator) => format!("TypeOperator({:?})", type_operator),
        TsType::TsImportType(import_type) => format!("ImportType({:?})", import_type),
        TsType::TsParenthesizedType(parenthesized_type) => {
            format!("ParenthesizedType({:?})", parenthesized_type)
        }
        TsType::TsInferType(infer_type) => format!("InferType({:?})", infer_type),
        TsType::TsThisType(this_type) => format!("ThisType({:?})", this_type),
        TsType::TsOptionalType(optional_type) => format!("OptionalType({:?})", optional_type),
        TsType::TsRestType(rest_type) => format!("RestType({:?})", rest_type),
        TsType::TsLitType(lit_type) => format!("LitType({:?})", lit_type),
        TsType::TsTypePredicate(type_predicate) => {
            format!("TypePredicate({:?})", type_predicate)
        }
    }
}
