use ::value::Value;
use primitive_calling_convention::primitive_calling_convention;
use vrl::prelude::*;

use crate::util::round_to_precision;

fn floor(value: Value, precision: Option<Value>) -> Resolved {
    let precision = match precision {
        Some(value) => value.try_integer()?,
        None => 0,
    };
    match value {
        Value::Float(f) => Ok(Value::from_f64_or_zero(round_to_precision(
            *f,
            precision,
            f64::floor,
        ))),
        value @ Value::Integer(_) => Ok(value),
        value => Err(value::Error::Expected {
            got: value.kind(),
            expected: Kind::float() | Kind::integer(),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Floor;

impl Function for Floor {
    fn identifier(&self) -> &'static str {
        "floor"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::ANY,
                required: true,
            },
            Parameter {
                keyword: "precision",
                kind: kind::ANY,
                required: false,
            },
        ]
    }

    fn compile(
        &self,
        _state: (&mut state::LocalEnv, &mut state::ExternalEnv),
        _ctx: &mut FunctionCompileContext,
        mut arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let precision = arguments.optional("precision");

        Ok(Box::new(FloorFn { value, precision }))
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "floor",
            source: r#"floor(9.8)"#,
            result: Ok("9.0"),
        }]
    }

    fn symbol(&self) -> Option<Symbol> {
        Some(Symbol {
            name: "vrl_fn_floor",
            address: vrl_fn_floor as _,
            uses_context: false,
        })
    }
}

#[derive(Clone, Debug)]
struct FloorFn {
    value: Box<dyn Expression>,
    precision: Option<Box<dyn Expression>>,
}

impl Expression for FloorFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let precision = self
            .precision
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;

        floor(value, precision)
    }

    fn type_def(&self, state: (&state::LocalEnv, &state::ExternalEnv)) -> TypeDef {
        match Kind::from(self.value.type_def(state)) {
            v if v.is_float() || v.is_integer() => v.into(),
            _ => Kind::integer().or_float().into(),
        }
    }
}

#[no_mangle]
#[primitive_calling_convention]
extern "C" fn vrl_fn_floor(value: Value, precision: Option<Value>) -> Resolved {
    floor(value, precision)
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        floor => Floor;

        lower {
            args: func_args![value: 1234.2],
            want: Ok(value!(1234.0)),
            tdef: TypeDef::float(),
        }

        higher {
            args: func_args![value: 1234.8],
            want: Ok(value!(1234.0)),
            tdef: TypeDef::float(),
        }

        exact {
            args: func_args![value: 1234],
            want: Ok(value!(1234)),
            tdef: TypeDef::integer(),
        }

        precision {
            args: func_args![value: 1234.39429,
                             precision: 1],
            want: Ok(value!(1234.3)),
            tdef: TypeDef::float(),
        }

        bigger_precision {
            args: func_args![value: 1234.56789,
                             precision: 4],
            want: Ok(value!(1234.5678)),
            tdef: TypeDef::float(),
        }

        huge_number {
            args: func_args![value: 9876543210123456789098765432101234567890987654321.987654321,
                             precision: 5],
            want: Ok(value!(9876543210123456789098765432101234567890987654321.98765)),
            tdef: TypeDef::float(),
        }
    ];
}
