/// A lexically-scoped symbol table.
///
/// Scopes are maintained as a stack of hash maps. The innermost scope is
/// always searched first so inner bindings shadow outer ones.
use std::collections::HashMap;

pub struct SymbolTable<T> {
    scopes: Vec<HashMap<String, T>>,
}

impl<T: Clone> Default for SymbolTable<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> SymbolTable<T> {
    /// Create a new symbol table with one (global) scope already open.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Open a new inner scope.
    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Close the innermost scope, discarding all bindings defined within it.
    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    /// Insert a binding into the **current** (innermost) scope.
    pub fn insert(&mut self, name: String, val: T) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, val);
        }
    }

    /// Look up a name, searching from the innermost scope outward.
    pub fn lookup(&self, name: &str) -> Option<T> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    /// Look up a name only in the **current** (innermost) scope.
    pub fn lookup_current(&self, name: &str) -> Option<T> {
        self.scopes.last().and_then(|s| s.get(name).cloned())
    }
}
