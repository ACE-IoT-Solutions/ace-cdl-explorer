use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, fmt};

/// Expanded class IRI (CXF §8.2). Prefix expansion belongs to format adapters.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClassId(pub String);
impl From<&str> for ClassId {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}
impl From<String> for ClassId {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl Borrow<str> for ClassId {
    fn borrow(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ClassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// JSON-shaped annotation data, without a JSON-library dependency.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Annotation {
    /// No value.
    Null,
    /// Boolean literal.
    Boolean(bool),
    /// Signed integer literal.
    Integer(i64),
    /// Unsigned integer that may exceed the signed range.
    Unsigned(u64),
    /// Real literal.
    Real(f64),
    /// Text or an uninterpreted IRI.
    String(String),
    /// Ordered list.
    Array(Vec<Annotation>),
    /// Ordered property map.
    Object(IndexMap<String, Annotation>),
}

/// Labels, descriptions and open namespaced properties (CXF §8.2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// Human-readable label.
    pub label: Option<String>,
    /// Human-readable documentation.
    pub description: Option<String>,
    /// Properties owned by the originating format or an extension namespace.
    #[serde(default)]
    pub annotations: IndexMap<String, Annotation>,
}

/// Connector or declaration data type (CXF §8.2, table 8.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// Continuous numeric value.
    Real,
    /// Signed integral value.
    Integer,
    /// Logical value.
    Boolean,
    /// Text value.
    String,
    /// Member of a declared enumeration.
    Enumeration(ClassId),
}

/// Typed literal or preserved expression (CXF §8.6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Value {
    /// Floating point scalar.
    Real(f64),
    /// Integral scalar.
    Integer(i64),
    /// Logical scalar.
    Boolean(bool),
    /// Literal text, distinct from an expression.
    String(String),
    /// Enumeration member name.
    Enum(String),
    /// Array, including nested arrays.
    Array(Vec<Value>),
    /// Expression preserved without evaluation.
    Expr(String),
}

/// A preserved array dimension (CXF §8.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dimension {
    /// Statically known positive extent.
    Size(u64),
    /// Unevaluated extent.
    Expr(String),
}

/// QUDT units and quantity kind; values are not converted by the model.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Units {
    /// QUDT unit IRI.
    pub unit: Option<String>,
    /// Display unit IRI (`S231:hasDisplayUnit`).
    pub display_unit: Option<String>,
    /// Quantity-kind IRI.
    pub quantity_kind: Option<String>,
}

/// Direction relative to the connector's owning block (CXF §8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Input to the owning block.
    Input,
    /// Output from the owning block.
    Output,
}

/// Typed connector declaration (`hasInput`/`hasOutput`, CXF §8.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    /// Local name.
    pub name: String,
    /// Owning-block direction.
    pub direction: Direction,
    /// Carried data type.
    pub data_type: DataType,
    /// Declared units.
    #[serde(default)]
    pub units: Units,
    /// Array dimensions, empty for scalars.
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    /// Whether a child input requires a driver; optional inputs are registry policy.
    pub required: bool,
    /// Documentation and extension properties.
    #[serde(default)]
    pub meta: Meta,
}
impl Connector {
    /// Construct a required scalar connector.
    pub fn new(name: impl Into<String>, direction: Direction, data_type: DataType) -> Self {
        Self {
            name: name.into(),
            direction,
            data_type,
            units: Units::default(),
            dimensions: Vec::new(),
            required: true,
            meta: Meta::default(),
        }
    }
}

/// Parameter or constant declaration (CXF §8.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterDecl {
    /// Local name.
    pub name: String,
    /// Declared type.
    pub data_type: DataType,
    /// Default binding or unevaluated expression.
    pub default: Option<Value>,
    /// Units and quantity kind.
    #[serde(default)]
    pub units: Units,
    /// Lower bound, potentially an expression.
    pub min: Option<Value>,
    /// Upper bound, potentially an expression.
    pub max: Option<Value>,
    /// Array dimensions.
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    /// Documentation and additional properties.
    #[serde(default)]
    pub meta: Meta,
}

/// Instance parameter binding (`hasInstance`/`value`, CXF §8.2 and §8.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    /// Parameter's local name.
    pub name: String,
    /// Value or preserved expression.
    pub value: Value,
    /// Binding-specific annotations, such as `isFinal`.
    #[serde(default)]
    pub meta: Meta,
}

/// Class instance, distinct from the class definition (CXF §8.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    /// Local instance name.
    pub name: String,
    /// Referenced class.
    pub class: ClassId,
    /// Parameter bindings.
    #[serde(default)]
    pub assignments: Vec<Assignment>,
    /// Instance connectors absent from or overriding a class declaration.
    #[serde(default)]
    pub connectors: Vec<Connector>,
    /// Instance-specific parameter declaration metadata or interface additions.
    #[serde(default)]
    pub parameters: Vec<ParameterDecl>,
    /// Instance documentation and annotations.
    #[serde(default)]
    pub meta: Meta,
}
impl Instance {
    /// Construct an unconfigured instance.
    pub fn new(name: impl Into<String>, class: impl Into<ClassId>) -> Self {
        Self {
            name: name.into(),
            class: class.into(),
            assignments: Vec::new(),
            connectors: Vec::new(),
            parameters: Vec::new(),
            meta: Meta::default(),
        }
    }
}

/// Connector reference relative to one composite body (CXF §8.2 and §8.5).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    /// Immediate child instance, or `None` for the enclosing block boundary.
    pub instance: Option<String>,
    /// Local connector name.
    pub connector: String,
}
impl Endpoint {
    /// Reference an enclosing block's connector.
    pub fn boundary(connector: impl Into<String>) -> Self {
        Self {
            instance: None,
            connector: connector.into(),
        }
    }
    /// Reference an immediate child's connector.
    pub fn child(instance: impl Into<String>, connector: impl Into<String>) -> Self {
        Self {
            instance: Some(instance.into()),
            connector: connector.into(),
        }
    }
    /// Parse compatibility-style `instance.connector` or a boundary name.
    pub fn parse(path: &str) -> Self {
        match path.split_once('.') {
            Some((instance, connector)) => Self::child(instance, connector),
            None => Self::boundary(path),
        }
    }
}
impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(instance) = &self.instance {
            write!(f, "{instance}.")?;
        }
        self.connector.fmt(f)
    }
}

/// Unoriented wire; direction follows connector roles (CXF §8.2 `connectedTo`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    /// One end of the wire.
    pub a: Endpoint,
    /// Other end of the wire.
    pub b: Endpoint,
    /// Preserved wire properties, including compatibility spelling.
    #[serde(default)]
    pub meta: Meta,
}

/// Composite implementation, separate from each use of the class (CXF §8.2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CompositeBody {
    /// Children, in declaration order.
    pub instances: Vec<Instance>,
    /// Connections between children and the enclosing interface.
    pub connections: Vec<Connection>,
}

/// Executable block class category (CXF §8.2, table 8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    /// Standard elementary implementation.
    Elementary,
    /// Wired composition of instances.
    Composite,
    /// Explicit nonstandard implementation.
    Extension,
}

/// Block class definition and interface (CXF §8.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockClass {
    /// Class IRI.
    pub id: ClassId,
    /// Implementation category.
    pub kind: BlockKind,
    /// Input declarations.
    #[serde(default)]
    pub inputs: Vec<Connector>,
    /// Output declarations.
    #[serde(default)]
    pub outputs: Vec<Connector>,
    /// Parameter declarations.
    #[serde(default)]
    pub parameters: Vec<ParameterDecl>,
    /// Constant declarations.
    #[serde(default)]
    pub constants: Vec<ParameterDecl>,
    /// Present exactly for composites.
    pub body: Option<CompositeBody>,
    /// Whether the implementation's output breaks an algebraic dependency loop.
    #[serde(default)]
    pub breaks_algebraic_loop: bool,
    /// Class metadata.
    #[serde(default)]
    pub meta: Meta,
}
impl BlockClass {
    /// Create an empty class, with an empty body for composites.
    pub fn new(id: impl Into<ClassId>, kind: BlockKind) -> Self {
        Self {
            id: id.into(),
            kind,
            inputs: Vec::new(),
            outputs: Vec::new(),
            parameters: Vec::new(),
            constants: Vec::new(),
            body: (kind == BlockKind::Composite).then(CompositeBody::default),
            breaks_algebraic_loop: false,
            meta: Meta::default(),
        }
    }
    /// Find a declared connector by local name.
    pub fn connector(&self, name: &str) -> Option<&Connector> {
        self.inputs
            .iter()
            .chain(&self.outputs)
            .find(|port| port.name == name)
    }
}

/// Enumeration class definition (CXF §8.2 `EnumerationType`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Enumeration {
    /// Expanded class IRI.
    pub id: ClassId,
    /// Ordered member names, corresponding to integer values starting at one.
    pub members: Vec<String>,
    /// Documentation and properties.
    #[serde(default)]
    pub meta: Meta,
}

/// Ordered class definitions, reusable across many instances (CXF §8.2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Library {
    /// Executable block definitions.
    pub classes: IndexMap<ClassId, BlockClass>,
    /// Enumeration definitions.
    pub enumerations: IndexMap<ClassId, Enumeration>,
    /// Package and other non-executable metadata.
    #[serde(default)]
    pub meta: Meta,
}

/// Configured root instance and the library resolving its references (CXF §8.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Available class definitions.
    pub library: Library,
    /// Root class instance.
    pub root: Instance,
}
