use aec_ast::{Program, ResolvedImport, TopLevelItem};
use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

pub struct LoadedProgram {
    pub program: Program,
    pub sources: Vec<SourceFile>,
    pub origins: Vec<usize>,
}

#[derive(Clone)]
struct ImportedSnapshot {
    items: Vec<TopLevelItem>,
    item_modules: Vec<Option<String>>,
    origins: Vec<usize>,
}

pub fn load_program(path: &Path) -> Result<LoadedProgram> {
    let mut loader = Loader {
        sources: Vec::new(),
        origins: Vec::new(),
        item_modules: Vec::new(),
        imports: Vec::new(),
        exports_by_path: std::collections::HashMap::new(),
        snapshots: std::collections::HashMap::new(),
        declarations: std::collections::HashMap::new(),
        seen_aliases: HashSet::new(),
        seen_unaliased: HashSet::new(),
        visited: HashSet::new(),
        active: Vec::new(),
    };
    let (header, span, items) = loader.load(path, true, None)?;
    Ok(LoadedProgram {
        program: Program {
            header,
            span,
            items,
            imports: loader.imports,
            item_modules: loader.item_modules,
        },
        sources: loader.sources,
        origins: loader.origins,
    })
}

struct Loader {
    sources: Vec<SourceFile>,
    origins: Vec<usize>,
    item_modules: Vec<Option<String>>,
    imports: Vec<ResolvedImport>,
    exports_by_path: std::collections::HashMap<PathBuf, Vec<String>>,
    snapshots: std::collections::HashMap<PathBuf, ImportedSnapshot>,
    declarations: std::collections::HashMap<(u8, String, Option<String>), PathBuf>,
    seen_aliases: HashSet<(PathBuf, String)>,
    seen_unaliased: HashSet<(PathBuf, Option<String>)>,
    visited: HashSet<PathBuf>,
    active: Vec<PathBuf>,
}

impl Loader {
    fn register_declaration(
        &mut self,
        item: &TopLevelItem,
        module_scope: Option<&str>,
        source: &Path,
    ) -> Result<()> {
        let Some((kind, name)) = declaration_identity(item) else {
            return Ok(());
        };
        let key = (kind, name.clone(), module_scope.map(str::to_string));
        if let Some(previous) = self.declarations.insert(key, source.to_path_buf()) {
            bail!(
                "duplicate {} '{}' in {} and {}",
                declaration_kind(kind),
                name,
                previous.display(),
                source.display()
            );
        }
        Ok(())
    }

    fn load(
        &mut self,
        path: &Path,
        is_root: bool,
        module_scope: Option<&str>,
    ) -> Result<(aec_ast::AgentHeader, aec_ast::Span, Vec<TopLevelItem>)> {
        let canonical = fs::canonicalize(path)
            .with_context(|| format!("cannot load {}", path.display()))?;
        let source = fs::read_to_string(&canonical)
            .with_context(|| format!("cannot read {}", canonical.display()))?;
        let program = aec_parser::parse(&source).map_err(|error| {
            let message = match &error.kind {
                aec_ast::ParseErrorKind::BuildError { message } => message.clone(),
                other => other.to_string(),
            };
            anyhow::anyhow!(
                "{}\n{}",
                canonical.display(),
                crate::render::render(&source, crate::render::Level::Error, &message, error.span)
            )
        })?;
        let source_index = self.sources.len();
        self.sources.push(SourceFile {
            path: canonical.clone(),
            text: source,
        });
        self.active.push(canonical.clone());
        self.visited.insert(canonical.clone());
        let mut items = Vec::new();
        for item in program.items {
            match item {
                TopLevelItem::Import(import) => {
                    let parent = canonical.parent().ok_or_else(|| {
                        anyhow::anyhow!("{} has no parent directory", canonical.display())
                    })?;
                    let imported = parent.join(&import.path);
                    let imported = fs::canonicalize(&imported).with_context(|| {
                        format!(
                            "{}:{}: cannot import {}",
                            canonical.display(),
                            import.span.start.line,
                            import.path
                        )
                    })?;
                    if self.active.contains(&imported) {
                        let chain = self
                            .active
                            .iter()
                            .chain(std::iter::once(&imported))
                            .map(|path| path.display().to_string())
                            .collect::<Vec<_>>()
                            .join(" -> ");
                        bail!("circular import: {chain}");
                    }
                    let alias = import.alias.as_ref().map(|alias| alias.name.clone());
                    if !self.visited.contains(&imported) {
                        let child_scope = alias.as_ref().map(|name| match module_scope {
                            Some(parent) => format!("{parent}.{name}"),
                            None => name.clone(),
                        });
                        let child_scope_ref = child_scope.as_deref().or(module_scope);
                        let child_item_start = self.item_modules.len();
                        let child_origin_start = self.origins.len();
                        let (_, _, imported_items) = self.load(&imported, false, child_scope_ref)?;
                        let child_item_end = self.item_modules.len();
                        let child_origin_end = self.origins.len();
                        let raw_modules = self.item_modules[child_item_start..child_item_end]
                            .iter()
                            .map(|module| strip_outer_scope(module, child_scope_ref))
                            .collect();
                        self.snapshots.insert(
                            imported.clone(),
                            ImportedSnapshot {
                                items: imported_items.clone(),
                                item_modules: raw_modules,
                                origins: self.origins[child_origin_start..child_origin_end].to_vec(),
                            },
                        );
                        let exports = exported_names(&imported_items);
                        self.exports_by_path.insert(imported.clone(), exports.clone());
                        if let Some(name) = alias.as_ref() {
                            self.seen_aliases.insert((imported.clone(), name.clone()));
                        } else {
                            self.seen_unaliased.insert((
                                imported.clone(),
                                module_scope.map(str::to_string),
                            ));
                        }
                        self.imports.push(ResolvedImport {
                            alias: alias.clone(),
                            path: import.path.clone(),
                            exports,
                        });
                        items.extend(imported_items);
                    } else if let Some(exports) = self.exports_by_path.get(&imported).cloned() {
                        self.imports.push(ResolvedImport {
                            alias: alias.clone(),
                            path: import.path.clone(),
                            exports,
                        });
                        if let Some(name) = alias.as_ref() {
                            if self.seen_aliases.insert((imported.clone(), name.clone())) {
                                if let Some(snapshot) = self.snapshots.get(&imported).cloned() {
                                    let prefix = match module_scope {
                                        Some(parent) => format!("{parent}.{name}"),
                                        None => name.clone(),
                                    };
                                    for ((item, module), origin) in snapshot
                                        .items
                                        .iter()
                                        .zip(snapshot.item_modules.iter())
                                        .zip(snapshot.origins.iter())
                                    {
                                        let scoped = add_outer_scope(module, Some(&prefix));
                                        self.register_declaration(item, scoped.as_deref(), &imported)?;
                                        self.item_modules.push(scoped);
                                        self.origins.push(*origin);
                                    }
                                    items.extend(snapshot.items);
                                }
                            }
                        } else if self
                            .seen_unaliased
                            .insert((imported.clone(), module_scope.map(str::to_string)))
                        {
                            if let Some(snapshot) = self.snapshots.get(&imported).cloned() {
                                for ((item, module), origin) in snapshot
                                    .items
                                    .iter()
                                    .zip(snapshot.item_modules.iter())
                                    .zip(snapshot.origins.iter())
                                {
                                    let scoped = add_outer_scope(module, module_scope);
                                    self.register_declaration(item, scoped.as_deref(), &imported)?;
                                    self.item_modules.push(scoped);
                                    self.origins.push(*origin);
                                }
                                items.extend(snapshot.items);
                            }
                        }
                    }
                }
                TopLevelItem::Permissions(_) | TopLevelItem::Limits(_) if !is_root => {
                    bail!(
                        "{}: permissions and limits must be declared in the entry file",
                        canonical.display()
                    );
                }
                other => {
                    self.register_declaration(&other, module_scope, &canonical)?;
                    items.push(other);
                    self.origins.push(source_index);
                    self.item_modules.push(module_scope.map(str::to_string));
                }
            }
        }
        self.active.pop();
        Ok((program.header, program.span, items))
    }
}

fn strip_outer_scope(module: &Option<String>, outer: Option<&str>) -> Option<String> {
    let Some(outer) = outer else {
        return module.clone();
    };
    let Some(module) = module else {
        return None;
    };
    if module == outer {
        None
    } else {
        module.strip_prefix(&format!("{outer}.")).map(str::to_string)
    }
}

fn add_outer_scope(module: &Option<String>, outer: Option<&str>) -> Option<String> {
    let Some(outer) = outer else {
        return module.clone();
    };
    Some(match module {
        Some(module) => format!("{outer}.{module}"),
        None => outer.to_string(),
    })
}

fn declaration_identity(item: &TopLevelItem) -> Option<(u8, String)> {
    match item {
        TopLevelItem::Function(declaration) => Some((0, declaration.name.name.clone())),
        TopLevelItem::Model(declaration) => Some((1, declaration.name.name.clone())),
        TopLevelItem::TypeAlias(declaration) => Some((5, declaration.name.name.clone())),
        TopLevelItem::Component(declaration) => Some((2, declaration.name.name.clone())),
        TopLevelItem::Theme(declaration) => Some((3, declaration.name.name.clone())),
        TopLevelItem::Ui(declaration) => Some((4, declaration.name.name.clone())),
        _ => None,
    }
}

fn declaration_kind(kind: u8) -> &'static str {
    match kind {
        0 => "function",
        1 => "model",
        2 => "component",
        3 => "theme",
        4 => "UI",
        5 => "type alias",
        _ => "declaration",
    }
}

fn exported_names(items: &[TopLevelItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            TopLevelItem::Function(declaration) if declaration.is_public => {
                Some(declaration.name.name.clone())
            }
            TopLevelItem::Model(declaration) if declaration.is_public => {
                Some(declaration.name.name.clone())
            }
            TopLevelItem::TypeAlias(declaration) if declaration.is_public => {
                Some(declaration.name.name.clone())
            }
            TopLevelItem::Component(declaration) if declaration.is_public => {
                Some(declaration.name.name.clone())
            }
            TopLevelItem::Theme(declaration) if declaration.is_public => {
                Some(declaration.name.name.clone())
            }
            TopLevelItem::Ui(declaration) if declaration.is_public => {
                Some(declaration.name.name.clone())
            }
            _ => None,
        })
        .collect()
}
