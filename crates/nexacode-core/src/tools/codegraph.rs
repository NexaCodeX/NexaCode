use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use async_trait::async_trait;
use rusqlite::{params, Connection};
use serde_json::Value;
use regex::Regex;

use super::traits::Tool;
use super::types::{RiskLevel, ToolContext, ToolDefinition, ToolResult};

pub struct CodeGraphTool;

impl CodeGraphTool {
    pub fn new() -> Self {
        Self
    }

    // Helper to get SQLite connection for current workspace
    fn get_db_conn(&self, context: &ToolContext) -> Result<Connection, String> {
        let nexacode_dir = context.working_dir.join(".nexacode");
        std::fs::create_dir_all(&nexacode_dir)
            .map_err(|e| format!("Failed to create .nexacode directory: {}", e))?;
        
        let db_path = nexacode_dir.join("codegraph.db");
        let conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open SQLite database: {}", e))?;

        // Initialize tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                docstring TEXT
            )",
            [],
        ).map_err(|e| format!("Failed to create nodes table: {}", e))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS edges (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                PRIMARY KEY (source_id, target_id, kind),
                FOREIGN KEY (source_id) REFERENCES nodes(id) ON DELETE CASCADE,
                FOREIGN KEY (target_id) REFERENCES nodes(id) ON DELETE CASCADE
            )",
            [],
        ).map_err(|e| format!("Failed to create edges table: {}", e))?;

        Ok(conn)
    }

    // Walks workspace recursively, ignoring common large directories
    fn collect_files(&self, dir: &Path, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !matches!(name, "node_modules" | ".git" | "target" | "dist" | ".next" | "build" | ".svelte-kit" | ".gemini" | "out") {
                        self.collect_files(&path, files);
                    }
                } else if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if matches!(ext, "ts" | "tsx" | "js" | "jsx" | "rs" | "py" | "go") {
                            files.push(path);
                        }
                    }
                }
            }
        }
    }
}

struct ParsedSymbol {
    name: String,
    kind: String, // class, function, method, struct, interface
    start_line: usize,
    end_line: usize,
}

// Regex-based lightweight AST parser for symbols
fn parse_file_symbols(file_path: &Path, content: &str) -> Vec<ParsedSymbol> {
    let mut symbols = Vec::new();
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let lines: Vec<&str> = content.lines().collect();

    // Compile regex patterns once
    let re_rust_fn = Regex::new(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)").unwrap();
    let re_rust_struct = Regex::new(r"^\s*(?:pub\s+)?(?:struct|enum|trait)\s+([a-zA-Z0-9_]+)").unwrap();
    
    let re_ts_class = Regex::new(r"^\s*(?:export\s+)?(?:class|interface)\s+([a-zA-Z0-9_]+)").unwrap();
    let re_ts_fn = Regex::new(r"^\s*(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z0-9_]+)").unwrap();
    let re_ts_const_fn = Regex::new(r"^\s*(?:export\s+)?const\s+([a-zA-Z0-9_]+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>").unwrap();

    let re_py_class = Regex::new(r"^\s*class\s+([a-zA-Z0-9_]+)").unwrap();
    let re_py_fn = Regex::new(r"^\s*def\s+([a-zA-Z0-9_]+)").unwrap();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        match ext {
            "rs" => {
                if let Some(cap) = re_rust_fn.captures(line) {
                    symbols.push(ParsedSymbol {
                        name: cap[1].to_string(),
                        kind: "function".to_string(),
                        start_line: line_num,
                        end_line: line_num,
                    });
                } else if let Some(cap) = re_rust_struct.captures(line) {
                    symbols.push(ParsedSymbol {
                        name: cap[1].to_string(),
                        kind: "struct".to_string(),
                        start_line: line_num,
                        end_line: line_num,
                    });
                }
            }
            "ts" | "tsx" | "js" | "jsx" => {
                if let Some(cap) = re_ts_class.captures(line) {
                    symbols.push(ParsedSymbol {
                        name: cap[1].to_string(),
                        kind: "class".to_string(),
                        start_line: line_num,
                        end_line: line_num,
                    });
                } else if let Some(cap) = re_ts_fn.captures(line) {
                    symbols.push(ParsedSymbol {
                        name: cap[1].to_string(),
                        kind: "function".to_string(),
                        start_line: line_num,
                        end_line: line_num,
                    });
                } else if let Some(cap) = re_ts_const_fn.captures(line) {
                    symbols.push(ParsedSymbol {
                        name: cap[1].to_string(),
                        kind: "function".to_string(),
                        start_line: line_num,
                        end_line: line_num,
                    });
                }
            }
            "py" => {
                if let Some(cap) = re_py_class.captures(line) {
                    symbols.push(ParsedSymbol {
                        name: cap[1].to_string(),
                        kind: "class".to_string(),
                        start_line: line_num,
                        end_line: line_num,
                    });
                } else if let Some(cap) = re_py_fn.captures(line) {
                    let name = cap[1].to_string();
                    let kind = if line.starts_with("    ") { "method" } else { "function" };
                    symbols.push(ParsedSymbol {
                        name,
                        kind: kind.to_string(),
                        start_line: line_num,
                        end_line: line_num,
                    });
                }
            }
            _ => {}
        }
    }

    // Assign rough end_lines based on the next symbol's start line
    let len = symbols.len();
    for i in 0..len {
        if i + 1 < len {
            symbols[i].end_line = symbols[i + 1].start_line - 1;
        } else {
            symbols[i].end_line = lines.len();
        }
    }

    symbols
}

#[async_trait]
impl Tool for CodeGraphTool {
    fn name(&self) -> &str {
        "CodeGraph"
    }

    fn description(&self) -> &str {
        "CRITICAL: Always use this tool FIRST when searching for code symbols, finding where a function/class/struct/interface is defined, \
         exploring call hierarchies, or analyzing file-to-file imports in the workspace. It is much faster and cheaper than using LS, Grep, or Bash. \
         Actions: 'find_definition' (query symbol name), 'find_references' (query symbol name to find callers), \
         'get_call_hierarchy' (trace callers/callees), 'get_file_dependencies' (imports list), 'index' (re-index)."
    }

    fn parameters(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["index", "find_definition", "find_references", "get_call_hierarchy", "get_file_dependencies", "list_nodes"],
                        "description": "The action to execute"
                    },
                    "query": {
                        "type": "string",
                        "description": "Symbol name or path query (required for find_definition, find_references)"
                    },
                    "symbol_id": {
                        "type": "string",
                        "description": "Target symbol ID (required for get_call_hierarchy)"
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["callers", "callees"],
                        "description": "Call direction (required for get_call_hierarchy)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Relative file path (required for get_file_dependencies)"
                    }
                },
                "required": ["action"]
            }),
        }
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    async fn execute(&self, args: Value, context: &ToolContext) -> ToolResult {
        let action = args["action"].as_str().unwrap_or("");
        let mut conn = match self.get_db_conn(context) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        match action {
            "index" => {
                let mut files = Vec::new();
                self.collect_files(&context.working_dir, &mut files);

                let tx = match conn.transaction() {
                    Ok(t) => t,
                    Err(e) => return ToolResult::error(format!("Failed to start transaction: {}", e)),
                };

                // Clear previous graphs
                let _ = tx.execute("DELETE FROM nodes", []);
                let _ = tx.execute("DELETE FROM edges", []);

                let mut all_symbol_names = HashSet::new();
                let mut file_symbols_map = HashMap::new();

                // First pass: Read files and collect symbol definitions
                for file_path in &files {
                    let relative_path = file_path
                        .strip_prefix(&context.working_dir)
                        .unwrap_or(file_path)
                        .to_string_lossy()
                        .to_string();

                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        // Store the file node
                        let file_node_id = format!("file:{}", relative_path);
                        let _ = tx.execute(
                            "INSERT OR REPLACE INTO nodes (id, file_path, name, kind, start_line, end_line) VALUES (?, ?, ?, ?, ?, ?)",
                            params![file_node_id, relative_path, relative_path, "file", 1, content.lines().count()],
                        );

                        let parsed = parse_file_symbols(file_path, &content);
                        for sym in &parsed {
                            let sym_id = format!("{}:{}", relative_path, sym.name);
                            all_symbol_names.insert(sym.name.clone());

                            let _ = tx.execute(
                                "INSERT OR REPLACE INTO nodes (id, file_path, name, kind, start_line, end_line) VALUES (?, ?, ?, ?, ?, ?)",
                                params![sym_id, relative_path, sym.name, sym.kind, sym.start_line, sym.end_line],
                            );

                            // Add a defining edge from file to symbol
                            let _ = tx.execute(
                                "INSERT OR REPLACE INTO edges (source_id, target_id, kind) VALUES (?, ?, ?)",
                                params![file_node_id, sym_id, "defines"],
                            );
                        }

                        file_symbols_map.insert(relative_path, (content, parsed));
                    }
                }

                // Second pass: Find imports and method calls
                for (relative_path, (content, parsed)) in &file_symbols_map {
                    let lines: Vec<&str> = content.lines().collect();

                    // Parse imports (very basic ES6 & Rust module import matching)
                    let re_import = Regex::new(r#"import\s+.*from\s+['"]([^'"]+)['"]|use\s+([^;:\s]+)"#).unwrap();
                    let file_node_id = format!("file:{}", relative_path);

                    for line in &lines {
                        if let Some(cap) = re_import.captures(line) {
                            let imported = cap.get(1).or_else(|| cap.get(2))
                                .map(|m| m.as_str().to_string())
                                .unwrap_or_default();
                            if !imported.is_empty() {
                                let target_id = format!("file:{}", imported);
                                let _ = tx.execute(
                                    "INSERT OR REPLACE INTO edges (source_id, target_id, kind) VALUES (?, ?, ?)",
                                    params![file_node_id, target_id, "imports"],
                                );
                            }
                        }
                    }

                    // Parse function calls within each symbol body
                    for sym in parsed {
                        let sym_id = format!("{}:{}", relative_path, sym.name);
                        let start = sym.start_line.saturating_sub(1);
                        let end = sym.end_line.min(lines.len());

                        let mut body_text = String::new();
                        for line in lines.iter().take(end).skip(start) {
                            body_text.push_str(line);
                            body_text.push(' ');
                        }

                        // Check if any defined global symbols are referenced in this body text
                        for other_sym_name in &all_symbol_names {
                            if other_sym_name == &sym.name {
                                continue;
                            }
                            
                            // Check using word boundary regex
                            let pattern = format!(r"\b{}\b", regex::escape(other_sym_name));
                            if let Ok(re_call) = Regex::new(&pattern) {
                                if re_call.is_match(&body_text) {
                                    // Search for definition to link to
                                    let mut stmt = tx.prepare(
                                        "SELECT id FROM nodes WHERE name = ? AND kind != 'file'"
                                    ).unwrap();
                                    let mut rows = stmt.query(params![other_sym_name]).unwrap();
                                    while let Ok(Some(row)) = rows.next() {
                                        let target_id: String = row.get(0).unwrap();
                                        let _ = tx.execute(
                                            "INSERT OR REPLACE INTO edges (source_id, target_id, kind) VALUES (?, ?, ?)",
                                            params![sym_id, target_id, "calls"],
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                if let Err(e) = tx.commit() {
                    return ToolResult::error(format!("Failed to commit database transaction: {}", e));
                }

                ToolResult::success(format!("Indexed {} files and generated symbol relation graph.", files.len()))
            }
            "list_nodes" => {
                let mut stmt = match conn.prepare(
                    "SELECT id, file_path, name, kind, start_line, end_line FROM nodes"
                ) {
                    Ok(s) => s,
                    Err(e) => return ToolResult::error(e.to_string()),
                };

                let rows = stmt.query_map([], |row| {
                    Ok(serde_json::json!({
                        "id": row.get::<_, String>(0)?,
                        "file_path": row.get::<_, String>(1)?,
                        "name": row.get::<_, String>(2)?,
                        "kind": row.get::<_, String>(3)?,
                        "start_line": row.get::<_, i64>(4)?,
                        "end_line": row.get::<_, i64>(5)?
                    }))
                });

                match rows {
                    Ok(mapped) => {
                        let results: Vec<Value> = mapped.flatten().collect();
                        ToolResult::success(serde_json::to_string_pretty(&results).unwrap_or_default())
                    }
                    Err(e) => ToolResult::error(e.to_string()),
                }
            }
            "find_definition" => {
                let query = args["query"].as_str().unwrap_or("");
                if query.is_empty() {
                    return ToolResult::error("Missing required parameter: query");
                }

                let mut stmt = match conn.prepare(
                    "SELECT id, file_path, name, kind, start_line, end_line FROM nodes \
                     WHERE name LIKE ? AND kind != 'file' LIMIT 25"
                ) {
                    Ok(s) => s,
                    Err(e) => return ToolResult::error(e.to_string()),
                };

                let search_pattern = format!("%{}%", query);
                let rows = stmt.query_map(params![search_pattern], |row| {
                    Ok(serde_json::json!({
                        "id": row.get::<_, String>(0)?,
                        "file_path": row.get::<_, String>(1)?,
                        "name": row.get::<_, String>(2)?,
                        "kind": row.get::<_, String>(3)?,
                        "start_line": row.get::<_, i64>(4)?,
                        "end_line": row.get::<_, i64>(5)?
                    }))
                });

                match rows {
                    Ok(mapped) => {
                        let results: Vec<Value> = mapped.flatten().collect();
                        ToolResult::success(serde_json::to_string_pretty(&results).unwrap_or_default())
                    }
                    Err(e) => ToolResult::error(e.to_string()),
                }
            }
            "find_references" => {
                let query = args["query"].as_str().unwrap_or("");
                if query.is_empty() {
                    return ToolResult::error("Missing required parameter: query");
                }

                // Finds callers calling target symbol name
                let mut stmt = match conn.prepare(
                    "SELECT n.id, n.file_path, n.name, n.kind, n.start_line, n.end_line \
                     FROM nodes n \
                     JOIN edges e ON n.id = e.source_id \
                     JOIN nodes target ON e.target_id = target.id \
                     WHERE target.name = ? AND e.kind = 'calls'"
                ) {
                    Ok(s) => s,
                    Err(e) => return ToolResult::error(e.to_string()),
                };

                let rows = stmt.query_map(params![query], |row| {
                    Ok(serde_json::json!({
                        "caller_id": row.get::<_, String>(0)?,
                        "file_path": row.get::<_, String>(1)?,
                        "name": row.get::<_, String>(2)?,
                        "kind": row.get::<_, String>(3)?,
                        "line": row.get::<_, i64>(4)?
                    }))
                });

                match rows {
                    Ok(mapped) => {
                        let results: Vec<Value> = mapped.flatten().collect();
                        ToolResult::success(serde_json::to_string_pretty(&results).unwrap_or_default())
                    }
                    Err(e) => ToolResult::error(e.to_string()),
                }
            }
            "get_call_hierarchy" => {
                let symbol_id = args["symbol_id"].as_str().unwrap_or("");
                let direction = args["direction"].as_str().unwrap_or("callees");

                if symbol_id.is_empty() {
                    return ToolResult::error("Missing required parameter: symbol_id");
                }

                let query = if direction == "callers" {
                    // Who calls this symbol?
                    "SELECT n.id, n.file_path, n.name, n.kind FROM nodes n \
                     JOIN edges e ON n.id = e.source_id \
                     WHERE e.target_id = ? AND e.kind = 'calls'"
                } else {
                    // Who does this symbol call?
                    "SELECT n.id, n.file_path, n.name, n.kind FROM nodes n \
                     JOIN edges e ON n.id = e.target_id \
                     WHERE e.source_id = ? AND e.kind = 'calls'"
                };

                let mut stmt = match conn.prepare(query) {
                    Ok(s) => s,
                    Err(e) => return ToolResult::error(e.to_string()),
                };

                let rows = stmt.query_map(params![symbol_id], |row| {
                    Ok(serde_json::json!({
                        "id": row.get::<_, String>(0)?,
                        "file_path": row.get::<_, String>(1)?,
                        "name": row.get::<_, String>(2)?,
                        "kind": row.get::<_, String>(3)?
                    }))
                });

                match rows {
                    Ok(mapped) => {
                        let results: Vec<Value> = mapped.flatten().collect();
                        ToolResult::success(serde_json::to_string_pretty(&results).unwrap_or_default())
                    }
                    Err(e) => ToolResult::error(e.to_string()),
                }
            }
            "get_file_dependencies" => {
                let path = args["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return ToolResult::error("Missing required parameter: path");
                }

                let file_node_id = format!("file:{}", path);

                // Files imported by this file
                let mut stmt_imports = match conn.prepare(
                    "SELECT target_id FROM edges WHERE source_id = ? AND kind = 'imports'"
                ) {
                    Ok(s) => s,
                    Err(e) => return ToolResult::error(e.to_string()),
                };
                let rows_imports = stmt_imports.query_map(params![file_node_id], |row| {
                    let id: String = row.get(0)?;
                    Ok(id.replace("file:", ""))
                }).unwrap().flatten().collect::<Vec<String>>();

                // Files importing this file
                let mut stmt_imported_by = match conn.prepare(
                    "SELECT source_id FROM edges WHERE target_id = ? AND kind = 'imports'"
                ) {
                    Ok(s) => s,
                    Err(e) => return ToolResult::error(e.to_string()),
                };
                let rows_imported_by = stmt_imported_by.query_map(params![file_node_id], |row| {
                    let id: String = row.get(0)?;
                    Ok(id.replace("file:", ""))
                }).unwrap().flatten().collect::<Vec<String>>();

                let result = serde_json::json!({
                    "file": path,
                    "imports": rows_imports,
                    "imported_by": rows_imported_by
                });

                ToolResult::success(serde_json::to_string_pretty(&result).unwrap_or_default())
            }
            _ => ToolResult::error(format!("Unknown CodeGraph action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_codegraph_indexing() {
        let temp_dir = tempdir().unwrap();
        let project_dir = temp_dir.path().to_path_buf();

        // Create a dummy Rust file
        let main_rs = project_dir.join("main.rs");
        std::fs::write(&main_rs, r#"
            use std::path::Path;
            
            struct DummyStruct {
                val: i32
            }

            fn main() {
                let helper = my_helper_function();
            }

            fn my_helper_function() -> i32 {
                100
            }
        "#).unwrap();

        let tool = CodeGraphTool::new();
        let context = ToolContext::new(project_dir);

        // Run index action
        let index_res = tool.execute(serde_json::json!({
            "action": "index"
        }), &context).await;

        assert!(!index_res.is_error, "Indexing failed: {}", index_res.output);
        assert!(index_res.output.contains("Indexed 1 files"), "Index output: {}", index_res.output);

        // Run find_definition action
        let def_res = tool.execute(serde_json::json!({
            "action": "find_definition",
            "query": "my_helper_function"
        }), &context).await;

        assert!(!def_res.is_error, "Find definition failed: {}", def_res.output);
        assert!(def_res.output.contains("my_helper_function"), "Definition output: {}", def_res.output);
        assert!(def_res.output.contains("function"), "Definition output: {}", def_res.output);
    }
}
