use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use syn::{Fields, File, FnArg, ImplItem, Item, ReturnType, Type, Visibility};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustSymbolKind {
    Struct,
    Enum,
    Function,
    Module,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFieldInfo {
    pub name: String,
    pub type_name: String,
    pub is_public: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustMethodInfo {
    pub name: String,
    pub signature: String,
    pub return_type: Option<String>,
    pub is_public: bool,
    pub is_executable_candidate: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSymbol {
    pub name: String,
    pub kind: RustSymbolKind,
    pub file_path: String,
    pub doc_comment: Option<String>,
    pub fields: Vec<RustFieldInfo>,
    pub methods: Vec<RustMethodInfo>,
    pub is_executable: bool,
    pub primary_entrypoint: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectScanReport {
    pub root: PathBuf,
    pub symbols: Vec<RustSymbol>,
    pub files: Vec<String>,
}

impl ProjectScanReport {
    pub fn find_symbol(&self, name: &str) -> Option<&RustSymbol> {
        self.symbols.iter().find(|s| s.name == name)
    }

    pub fn find_by_file(&self, rel_path: &str) -> Option<&RustSymbol> {
        self.symbols.iter().find(|s| s.file_path == rel_path)
    }
}

pub struct RustScanner;

impl RustScanner {
    pub fn scan_project(root: &Path) -> Result<ProjectScanReport, CoreError> {
        let mut report = ProjectScanReport {
            root: root.to_path_buf(),
            symbols: Vec::new(),
            files: Vec::new(),
        };

        let mut rs_files = Vec::new();
        Self::collect_rs_files(root, &mut rs_files)?;

        if rs_files.is_empty() {
            return Err(CoreError::LayoutFailed(format!(
                "No Rust source files (.rs) found in project {:?}",
                root
            )));
        }

        // Sort files for deterministic ordering
        rs_files.sort();

        for file_path in rs_files {
            let rel_path = file_path
                .strip_prefix(root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .to_string();

            report.files.push(rel_path.clone());

            if let Ok(content) = fs::read_to_string(&file_path) {
                if let Ok(syntax_tree) = syn::parse_file(&content) {
                    let mut file_symbols = Self::extract_symbols_from_ast(&syntax_tree, &rel_path);
                    report.symbols.append(&mut file_symbols);
                }
            }
        }

        Ok(report)
    }

    fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), CoreError> {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let dir_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    // Skip hidden directories, target, node_modules, build, packaging
                    if dir_name.starts_with('.')
                        || dir_name == "target"
                        || dir_name == "node_modules"
                        || dir_name == "build"
                        || dir_name == "packaging"
                    {
                        continue;
                    }
                    Self::collect_rs_files(&path, files)?;
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let f_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if !f_name.starts_with("merm_exec_") {
                        files.push(path);
                    }
                }
            }
        }
        Ok(())
    }

    fn extract_symbols_from_ast(syntax_tree: &File, rel_path: &str) -> Vec<RustSymbol> {
        let mut symbols_map: HashMap<String, RustSymbol> = HashMap::new();
        let mut impl_methods_map: HashMap<String, Vec<RustMethodInfo>> = HashMap::new();

        for item in &syntax_tree.items {
            match item {
                Item::Struct(s) => {
                    let name = s.ident.to_string();
                    let doc = Self::extract_doc_comment(&s.attrs);
                    let mut fields = Vec::new();

                    match &s.fields {
                        Fields::Named(named) => {
                            for f in &named.named {
                                let f_name =
                                    f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                                let f_type = Self::type_to_string(&f.ty);
                                let is_pub = matches!(f.vis, Visibility::Public(_));
                                let f_doc = Self::extract_doc_comment(&f.attrs);
                                fields.push(RustFieldInfo {
                                    name: f_name,
                                    type_name: f_type,
                                    is_public: is_pub,
                                    comment: f_doc,
                                });
                            }
                        }
                        Fields::Unnamed(unnamed) => {
                            for (idx, f) in unnamed.unnamed.iter().enumerate() {
                                let f_type = Self::type_to_string(&f.ty);
                                let is_pub = matches!(f.vis, Visibility::Public(_));
                                fields.push(RustFieldInfo {
                                    name: format!("_{}", idx),
                                    type_name: f_type,
                                    is_public: is_pub,
                                    comment: None,
                                });
                            }
                        }
                        Fields::Unit => {}
                    }

                    symbols_map.insert(
                        name.clone(),
                        RustSymbol {
                            name,
                            kind: RustSymbolKind::Struct,
                            file_path: rel_path.to_string(),
                            doc_comment: doc,
                            fields,
                            methods: Vec::new(),
                            is_executable: false,
                            primary_entrypoint: None,
                        },
                    );
                }
                Item::Enum(e) => {
                    let name = e.ident.to_string();
                    let doc = Self::extract_doc_comment(&e.attrs);
                    let mut fields = Vec::new();

                    for variant in &e.variants {
                        let v_name = variant.ident.to_string();
                        let v_doc = Self::extract_doc_comment(&variant.attrs);
                        fields.push(RustFieldInfo {
                            name: v_name,
                            type_name: "variant".to_string(),
                            is_public: true,
                            comment: v_doc,
                        });
                    }

                    symbols_map.insert(
                        name.clone(),
                        RustSymbol {
                            name,
                            kind: RustSymbolKind::Enum,
                            file_path: rel_path.to_string(),
                            doc_comment: doc,
                            fields,
                            methods: Vec::new(),
                            is_executable: false,
                            primary_entrypoint: None,
                        },
                    );
                }
                Item::Impl(i) => {
                    let self_type_name = Self::type_to_string(&i.self_ty);
                    let mut methods = Vec::new();

                    for sub in &i.items {
                        if let ImplItem::Fn(f) = sub {
                            let m_name = f.sig.ident.to_string();
                            let is_pub = matches!(f.vis, Visibility::Public(_));
                            let ret_type = match &f.sig.output {
                                ReturnType::Default => None,
                                ReturnType::Type(_, ty) => Some(Self::type_to_string(ty)),
                            };
                            let sig = Self::format_fn_sig(&f.sig);
                            let doc = Self::extract_doc_comment(&f.attrs);

                            // Detect executable candidate: run, execute, test_*, main, or public fn
                            let is_candidate = m_name == "run"
                                || m_name == "execute"
                                || m_name == "main"
                                || m_name.starts_with("test_");

                            methods.push(RustMethodInfo {
                                name: m_name,
                                signature: sig,
                                return_type: ret_type,
                                is_public: is_pub,
                                is_executable_candidate: is_candidate,
                                comment: doc,
                            });
                        }
                    }

                    impl_methods_map
                        .entry(self_type_name)
                        .or_default()
                        .extend(methods);
                }
                Item::Fn(f) => {
                    let m_name = f.sig.ident.to_string();
                    let is_pub = matches!(f.vis, Visibility::Public(_));
                    let is_candidate = m_name == "main"
                        || m_name == "run"
                        || m_name == "execute"
                        || m_name.starts_with("test_");

                    let ret_type = match &f.sig.output {
                        ReturnType::Default => None,
                        ReturnType::Type(_, ty) => Some(Self::type_to_string(ty)),
                    };
                    let sig = Self::format_fn_sig(&f.sig);
                    let doc = Self::extract_doc_comment(&f.attrs);

                    // If standalone executable function or module entrypoint
                    if is_candidate || is_pub {
                        symbols_map.insert(
                            m_name.clone(),
                            RustSymbol {
                                name: m_name.clone(),
                                kind: RustSymbolKind::Function,
                                file_path: rel_path.to_string(),
                                doc_comment: doc.clone(),
                                fields: Vec::new(),
                                methods: vec![RustMethodInfo {
                                    name: m_name.clone(),
                                    signature: sig,
                                    return_type: ret_type,
                                    is_public: is_pub,
                                    is_executable_candidate: is_candidate,
                                    comment: doc,
                                }],
                                is_executable: is_candidate,
                                primary_entrypoint: Some(m_name),
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        // Attach impl methods to matching structs/enums
        for (type_name, methods) in impl_methods_map {
            if let Some(symbol) = symbols_map.get_mut(&type_name) {
                for m in methods {
                    if m.is_executable_candidate && symbol.primary_entrypoint.is_none() {
                        symbol.is_executable = true;
                        symbol.primary_entrypoint = Some(m.name.clone());
                    }
                    symbol.methods.push(m);
                }
            }
        }

        // If file has no struct/enum, create a Module symbol for the file
        if symbols_map.is_empty() {
            let file_stem = Path::new(rel_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "module".to_string());

            symbols_map.insert(
                file_stem.clone(),
                RustSymbol {
                    name: file_stem,
                    kind: RustSymbolKind::Module,
                    file_path: rel_path.to_string(),
                    doc_comment: None,
                    fields: Vec::new(),
                    methods: Vec::new(),
                    is_executable: true,
                    primary_entrypoint: Some("run".to_string()),
                },
            );
        }

        symbols_map.into_values().collect()
    }

    fn extract_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
        let mut lines = Vec::new();
        for attr in attrs {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                    {
                        lines.push(s.value().trim().to_string());
                    }
                }
            }
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join(" "))
        }
    }

    fn type_to_string(ty: &Type) -> String {
        quote::quote!(#ty).to_string().replace(' ', "")
    }

    fn format_fn_sig(sig: &syn::Signature) -> String {
        let name = &sig.ident;
        let mut params = Vec::new();
        for arg in &sig.inputs {
            match arg {
                FnArg::Receiver(r) => {
                    if r.reference.is_some() {
                        if r.mutability.is_some() {
                            params.push("&mut self".to_string());
                        } else {
                            params.push("&self".to_string());
                        }
                    } else {
                        params.push("self".to_string());
                    }
                }
                FnArg::Typed(pat) => {
                    let p_name = quote::quote!(#pat).to_string().replace(' ', "");
                    params.push(p_name);
                }
            }
        }
        format!("{}({})", name, params.join(", "))
    }

    pub fn generate_mermaid_class_diagram(report: &ProjectScanReport) -> String {
        let mut lines = Vec::new();
        lines.push("classDiagram".to_string());
        lines.push("    direction TD".to_string());
        lines.push("".to_string());

        for sym in &report.symbols {
            let stereotype = match sym.kind {
                RustSymbolKind::Struct => "<<struct>>",
                RustSymbolKind::Enum => "<<enum>>",
                RustSymbolKind::Module => "<<module>>",
                RustSymbolKind::Function => "<<fn>>",
            };

            lines.push(format!("    class {} {{", sym.name));
            lines.push(format!("        {}", stereotype));

            if let Some(ref doc) = sym.doc_comment {
                lines.push(format!("        %% {}", doc));
            }

            for f in &sym.fields {
                let vis = if f.is_public { "+" } else { "-" };
                let comm = f
                    .comment
                    .as_ref()
                    .map(|c| format!(" // {}", c))
                    .unwrap_or_default();
                lines.push(format!(
                    "        {}{}: {}{}",
                    vis, f.name, f.type_name, comm
                ));
            }

            for m in &sym.methods {
                let vis = if m.is_public { "+" } else { "-" };
                let ret = m
                    .return_type
                    .as_ref()
                    .map(|r| format!(" {}", r))
                    .unwrap_or_default();
                let comm = m
                    .comment
                    .as_ref()
                    .map(|c| format!(" // {}", c))
                    .unwrap_or_default();
                lines.push(format!("        {}{}{}{}", vis, m.signature, ret, comm));
            }

            lines.push("    }".to_string());
            lines.push(format!(
                "    %% @merm:node id=\"{}\" file=\"{}\" exec=\"{}\"",
                sym.name, sym.file_path, sym.is_executable
            ));
            lines.push("".to_string());
        }

        let mut known_symbols: HashSet<String> = HashSet::new();
        for sym in &report.symbols {
            known_symbols.insert(sym.name.clone());
        }

        // Infer relations between classes
        let mut seen_edges: HashSet<(String, String)> = HashSet::new();
        for sym in &report.symbols {
            for f in &sym.fields {
                for other in &known_symbols {
                    if other != &sym.name
                        && f.type_name.contains(other)
                        && seen_edges.insert((sym.name.clone(), other.clone()))
                    {
                        lines.push(format!("    {} *-- {} : {}", sym.name, other, f.name));
                    }
                }
            }
            for m in &sym.methods {
                for other in &known_symbols {
                    if other != &sym.name {
                        let in_sig = m.signature.contains(other);
                        let in_ret = m
                            .return_type
                            .as_ref()
                            .map(|r| r.contains(other))
                            .unwrap_or(false);
                        if (in_sig || in_ret)
                            && seen_edges.insert((sym.name.clone(), other.clone()))
                        {
                            lines.push(format!("    {} ..> {} : {}", sym.name, other, m.name));
                        }
                    }
                }
            }
        }

        lines.join("\n")
    }

    pub fn verify_diagram_compatibility(
        diagram_source: &str,
        report: &ProjectScanReport,
    ) -> CompatibilityReport {
        let mut report_out = CompatibilityReport {
            total_classes: 0,
            matched_classes: 0,
            missing_in_rust: Vec::new(),
            unbound_rust_symbols: Vec::new(),
            diagnostics: Vec::new(),
            is_compatible: true,
        };

        // Extract class names declared in diagram
        let mut diagram_classes = HashSet::new();
        for line in diagram_source.lines() {
            let t = line.trim();
            if t.starts_with("class ") {
                let rest = t.strip_prefix("class ").unwrap_or("").trim();
                let class_name = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('{')
                    .trim();
                if !class_name.is_empty() {
                    diagram_classes.insert(class_name.to_string());
                }
            }
        }

        report_out.total_classes = diagram_classes.len();

        for class_name in &diagram_classes {
            if let Some(sym) = report.find_symbol(class_name) {
                report_out.matched_classes += 1;
                report_out.diagnostics.push(format!(
                    "✔ [MATCH] Class '{}' binds to '{}' ({:?}, exec={})",
                    class_name, sym.file_path, sym.kind, sym.is_executable
                ));
            } else {
                report_out.missing_in_rust.push(class_name.clone());
                report_out.diagnostics.push(format!(
                    "✖ [MISSING] Class '{}' declared in diagram has no matching Rust symbol/file",
                    class_name
                ));
                report_out.is_compatible = false;
            }
        }

        for sym in &report.symbols {
            if !diagram_classes.contains(&sym.name) {
                report_out.unbound_rust_symbols.push(sym.name.clone());
            }
        }

        if !report_out.unbound_rust_symbols.is_empty() {
            report_out.diagnostics.push(format!(
                "ℹ [UNBOUND] Rust symbols not yet in diagram: {}",
                report_out.unbound_rust_symbols.join(", ")
            ));
        }

        report_out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub total_classes: usize,
    pub matched_classes: usize,
    pub missing_in_rust: Vec<String>,
    pub unbound_rust_symbols: Vec<String>,
    pub diagnostics: Vec<String>,
    pub is_compatible: bool,
}

impl CompatibilityReport {
    pub fn summary(&self) -> String {
        format!(
            "Compatibility: {}/{} matched | Status: {}",
            self.matched_classes,
            self.total_classes,
            if self.is_compatible { "OK" } else { "MISMATCH" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_rust_ast_snippet() {
        let code = r#"
        /// Authentication service handling user logins
        pub struct AuthService {
            pub user_id: u64,
            token: String,
        }

        impl AuthService {
            pub fn run(input: &str) -> String {
                format!("auth: {}", input)
            }
            
            pub fn logout(&self) {
            }
        }

        pub enum UserRole {
            Admin,
            Guest,
        }
        "#;

        let syntax = syn::parse_file(code).unwrap();
        let symbols = RustScanner::extract_symbols_from_ast(&syntax, "src/auth.rs");

        assert_eq!(symbols.len(), 2);
        let auth = symbols.iter().find(|s| s.name == "AuthService").unwrap();
        assert_eq!(auth.kind, RustSymbolKind::Struct);
        assert_eq!(auth.fields.len(), 2);
        assert_eq!(auth.methods.len(), 2);
        assert!(auth.is_executable);
        assert_eq!(auth.primary_entrypoint, Some("run".to_string()));

        let role = symbols.iter().find(|s| s.name == "UserRole").unwrap();
        assert_eq!(role.kind, RustSymbolKind::Enum);
        assert_eq!(role.fields.len(), 2);
    }

    #[test]
    fn test_scan_workspace_project() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
        let report = RustScanner::scan_project(workspace_root).expect("Should scan workspace");
        assert!(
            !report.symbols.is_empty(),
            "Should find symbols in workspace"
        );
        assert!(report.symbols.iter().any(|s| s.name == "RustScanner"));

        let mermaid = RustScanner::generate_mermaid_class_diagram(&report);
        assert!(mermaid.contains("class RustScanner"));
        assert!(mermaid.contains("classDiagram"));
    }
}
