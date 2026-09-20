pub mod ast_rewriter;
pub mod class_diagram;
pub mod engine;
pub mod error;
pub mod extractor;
pub mod quickjs_engine;
pub mod theme;
pub mod xml_utils;

pub use ast_rewriter::{AstRewriter, LayoutDirection};
pub use class_diagram::{ClassDef, ClassDiagramParser, ClassMember, ClassRelation, RelationKind};
pub use engine::{DiagramNode, LayoutEngine, RenderedDiagram};
pub use error::CoreError;
pub use extractor::{DiagramBlock, DiagramExtractor, DiagramType};
pub use quickjs_engine::QuickJsEngine;
pub use theme::{ColorPalette, ThemeId};
pub use xml_utils::{escape_xml, split_and_escape_lines};
