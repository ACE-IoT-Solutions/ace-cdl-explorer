use crate::{BlockClass, ClassId, Diagnostic, DiagnosticCode, Document, Instance};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Dot-separated instance path (CXF §8.5), with explicit segment operations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstancePath(pub Vec<String>);
impl InstancePath {
    /// A root path with one segment.
    pub fn root(name: impl Into<String>) -> Self {
        Self(vec![name.into()])
    }
    /// Append a child without modifying the original path.
    pub fn child(&self, name: impl Into<String>) -> Self {
        let mut path = self.clone();
        path.0.push(name.into());
        path
    }
    /// Parse a path, rejecting empty segments.
    pub fn parse(path: &str) -> Result<Self, Diagnostic> {
        if path.split('.').any(|segment| segment.is_empty()) {
            return Err(Diagnostic::new(
                path,
                DiagnosticCode::InvalidName,
                "empty instance path segment",
            ));
        }
        Ok(Self(path.split('.').map(str::to_owned).collect()))
    }
    /// Return the enclosing path, if one exists.
    pub fn parent(&self) -> Option<Self> {
        (self.0.len() > 1).then(|| Self(self.0[..self.0.len() - 1].to_vec()))
    }
}
impl fmt::Display for InstancePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.join(".").fmt(f)
    }
}

/// A path-resolved reference in a flattened instance traversal.
#[derive(Clone, Debug)]
pub struct FlatInstance<'a> {
    /// Full path, including the root.
    pub path: InstancePath,
    /// Configured instance.
    pub instance: &'a Instance,
    /// Resolved class definition.
    pub class: &'a BlockClass,
}
impl Document {
    /// Traverse all instances, root first, in declaration order.
    ///
    /// Shared classes are expanded independently. An active class stack rejects
    /// recursive instantiation without mistaking legitimate shared classes for cycles.
    pub fn instances(&self) -> Result<Vec<FlatInstance<'_>>, Vec<Diagnostic>> {
        let (result, errors) = self.walk();
        if errors.is_empty() {
            Ok(result)
        } else {
            Err(errors)
        }
    }

    pub(crate) fn walk(&self) -> (Vec<FlatInstance<'_>>, Vec<Diagnostic>) {
        let mut pending = vec![(
            &self.root,
            InstancePath::root(&self.root.name),
            Vec::<ClassId>::new(),
        )];
        let mut result = Vec::new();
        let mut errors = Vec::new();
        while let Some((instance, path, mut active)) = pending.pop() {
            let Some(class) = self.library.classes.get(&instance.class) else {
                errors.push(Diagnostic::new(
                    path.to_string(),
                    DiagnosticCode::UnknownClass,
                    format!("unknown class {}", instance.class),
                ));
                continue;
            };
            if active.contains(&instance.class) {
                errors.push(Diagnostic::new(
                    path.to_string(),
                    DiagnosticCode::RecursiveClass,
                    format!("recursive class {}", instance.class),
                ));
                continue;
            }
            active.push(instance.class.clone());
            result.push(FlatInstance {
                path: path.clone(),
                instance,
                class,
            });
            if let Some(body) = &class.body {
                for child in body.instances.iter().rev() {
                    pending.push((child, path.child(&child.name), active.clone()));
                }
            }
        }
        (result, errors)
    }
}
