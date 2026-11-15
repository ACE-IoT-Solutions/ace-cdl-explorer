use cdl_model::{
    Annotation, BlockClass, BlockKind, DataType, Diagnostic, DiagnosticCode, Library, Meta,
    ParameterDecl, Units, Value,
};

/// Explicit ACE editor interfaces for legacy behaviors absent from the pinned core,
/// plus the weighted Add and Gain aliases. This does not modify a registry's library.
/// Ports are derived from authoritative core interfaces; parameters here describe
/// editable initial values, which hosts bind explicitly when creating a block.
pub fn legacy_catalog_classes(core: &Library) -> Result<Vec<BlockClass>, Diagnostic> {
    type Variant<'a> = (&'a str, &'a str, &'a [(&'a str, Value)]);
    let variants: &[Variant<'_>] = &[
        (
            "Add",
            "Reals.Add",
            &[("k1", Value::Real(1.0)), ("k2", Value::Real(1.0))],
        ),
        (
            "Gain",
            "Reals.MultiplyByParameter",
            &[("k", Value::Real(1.0))],
        ),
        (
            "Delay",
            "Discrete.UnitDelay",
            &[("y_start", Value::Real(0.0))],
        ),
        (
            "LimPID",
            "Reals.PIDWithReset",
            &[
                ("k", Value::Real(0.5)),
                ("Ti", Value::Real(60.0)),
                ("yMin", Value::Real(0.0)),
                ("yMax", Value::Real(1.0)),
                ("reverseActing", Value::Boolean(false)),
            ],
        ),
        (
            "Filter",
            "Reals.MultiplyByParameter",
            &[("T", Value::Real(10.0))],
        ),
        (
            "LimRateLimiter",
            "Reals.MultiplyByParameter",
            &[
                ("riseRate", Value::Real(0.1)),
                ("fallRate", Value::Real(0.1)),
            ],
        ),
        ("Switch", "Reals.Switch", &[]),
    ];
    variants
        .iter()
        .map(|(family, base, parameters)| {
            let id: cdl_model::ClassId =
                format!("http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.{base}").into();
            let mut class = core.classes.get(&id).cloned().ok_or_else(|| {
                Diagnostic::new(
                    base.to_string(),
                    DiagnosticCode::UnknownClass,
                    "legacy catalog requires its core interface",
                )
            })?;
            class.id =
                format!("https://ace-iot-solutions.github.io/ace-cdl-explorer/cxf#editor.{family}")
                    .into();
            class.kind = BlockKind::Extension;
            class.body = None;
            class.meta = Meta::default();
            class.meta.label = Some((*family).into());
            class
                .meta
                .annotations
                .insert("ace:blockType".into(), Annotation::String((*family).into()));
            if *family == "Switch" {
                for p in &mut class.inputs {
                    if p.name == "u2" {
                        p.name = "u3".into();
                    } else if p.name == "u3" {
                        p.name = "u2".into();
                    }
                }
                class.inputs.sort_by(|a, b| a.name.cmp(&b.name));
            }
            if *family == "LimPID" {
                class.inputs.retain(|p| p.name != "trigger");
            }
            class.parameters = parameters
                .iter()
                .map(|(name, value)| {
                    let mut units = Units::default();
                    if matches!(*name, "T" | "Ti") {
                        units.unit = Some("http://qudt.org/vocab/unit#SEC".into());
                    }
                    ParameterDecl {
                        name: (*name).into(),
                        data_type: if matches!(value, Value::Boolean(_)) {
                            DataType::Boolean
                        } else {
                            DataType::Real
                        },
                        default: Some(value.clone()),
                        units,
                        min: None,
                        max: None,
                        dimensions: vec![],
                        meta: Meta::default(),
                    }
                })
                .collect();
            Ok(class)
        })
        .collect()
}
