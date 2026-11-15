use cdl_blocks::{ConfiguredInstance, KernelError, SlotAlloc};
use cdl_model::{Annotation, Diagnostic, DiagnosticCode, Value};
use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct Equations {
    parameters: HashMap<String, f64>,
    inputs: Vec<(String, usize)>,
    expressions: Vec<(String, meval::Expr, Option<usize>)>,
}
impl Equations {
    pub(crate) fn build(
        config: &ConfiguredInstance<'_>,
        io: &SlotAlloc,
        equations: &[Annotation],
    ) -> Result<Self, Diagnostic> {
        let error =
            |message: String| Diagnostic::new(config.path, DiagnosticCode::InvalidValue, message);
        let mut expressions = Vec::new();
        for equation in equations {
            let Annotation::Object(fields) = equation else {
                return Err(error("invalid equation record".into()));
            };
            let (Some(Annotation::String(lhs)), Some(Annotation::String(rhs))) =
                (fields.get("lhs"), fields.get("rhs"))
            else {
                return Err(error("equations require string lhs/rhs".into()));
            };
            let expression = rhs
                .parse::<meval::Expr>()
                .map_err(|e| error(e.to_string()))?;
            let slot = if config.class.outputs.iter().any(|p| &p.name == lhs) {
                Some(io.output(lhs, config.path)?)
            } else {
                None
            };
            expressions.push((lhs.clone(), expression, slot));
        }
        for output in &config.class.outputs {
            if !expressions.iter().any(|(name, _, _)| name == &output.name) {
                return Err(error(format!("no equation assigns output {}", output.name)));
            }
        }
        let parameters = config
            .instance
            .assignments
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    match p.value {
                        Value::Real(v) => v,
                        Value::Integer(v) => v as f64,
                        _ => 0.0, // Existing equation interpreter's numeric-parameter convention.
                    },
                )
            })
            .collect();
        let inputs = config
            .class
            .inputs
            .iter()
            .map(|p| Ok((p.name.clone(), io.input(&p.name, config.path)?)))
            .collect::<Result<_, Diagnostic>>()?;
        Ok(Self {
            parameters,
            inputs,
            expressions,
        })
    }
    pub(crate) fn step(&self, slots: &mut [f64]) -> Result<(), KernelError> {
        let mut vars = self.parameters.clone();
        for (name, index) in &self.inputs {
            vars.insert(name.clone(), slots[*index]);
        }
        for (name, expression, output) in &self.expressions {
            let mut ctx = meval::Context::new();
            for (name, value) in &vars {
                ctx.var(name, *value);
            }
            ctx.func2("min", f64::min);
            ctx.func2("max", f64::max);
            let value = expression
                .eval_with_context(ctx)
                .map_err(|e| KernelError(e.to_string()))?;
            vars.insert(name.clone(), value);
            if let Some(index) = output {
                slots[*index] = value;
            }
        }
        Ok(())
    }
}
