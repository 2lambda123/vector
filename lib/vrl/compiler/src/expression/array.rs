use std::{collections::BTreeMap, fmt, ops::Deref};

use value::Value;

use crate::{
    expression::{Expr, Resolved},
    state::{ExternalEnv, LocalEnv},
    Context, Expression, TypeDef,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    inner: Vec<Expr>,
}

impl Array {
    pub(crate) fn new(inner: Vec<Expr>) -> Self {
        Self { inner }
    }
}

impl Deref for Array {
    type Target = Vec<Expr>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Expression for Array {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.inner
            .iter()
            .map(|expr| expr.resolve(ctx))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }

    fn as_value(&self) -> Option<Value> {
        self.inner
            .iter()
            .map(Expr::as_value)
            .collect::<Option<Vec<_>>>()
            .map(Value::Array)
    }

    fn type_def(&self, state: (&LocalEnv, &ExternalEnv)) -> TypeDef {
        let type_defs = self
            .inner
            .iter()
            .map(|expr| expr.type_def(state))
            .collect::<Vec<_>>();

        // If any of the stored expressions is fallible, the entire array is
        // fallible.
        let fallible = type_defs.iter().any(TypeDef::is_fallible);

        let collection = type_defs
            .into_iter()
            .enumerate()
            .map(|(index, type_def)| (index.into(), type_def.into()))
            .collect::<BTreeMap<_, _>>();

        TypeDef::array(collection).with_fallibility(fallible)
    }

    #[cfg(feature = "llvm")]
    fn emit_llvm<'ctx>(
        &self,
        state: (&mut LocalEnv, &mut ExternalEnv),
        ctx: &mut crate::llvm::Context<'ctx>,
        function_call_abort_stack: &mut Vec<crate::llvm::BasicBlock<'ctx>>,
    ) -> Result<(), String> {
        let function = ctx.function();
        let begin_block = ctx.context().append_basic_block(function, "array_begin");
        ctx.builder().build_unconditional_branch(begin_block);
        ctx.builder().position_at_end(begin_block);

        let result_ref = ctx.result_ref();

        let end_block = ctx.context().append_basic_block(function, "array_end");

        let vec_ref = ctx.builder().build_alloca(ctx.vec_type(), "temp");
        ctx.vrl_vec_initialize().build_call(
            ctx.builder(),
            vec_ref,
            ctx.usize_type().const_int(self.inner.len() as _, false),
        );

        let insert_block = ctx.context().append_basic_block(function, "array_insert");
        ctx.builder().build_unconditional_branch(insert_block);
        ctx.builder().position_at_end(insert_block);

        for (index, value) in self.inner.iter().enumerate() {
            let value_ref = ctx.build_alloca_resolved("value");
            ctx.vrl_resolved_initialize()
                .build_call(ctx.builder(), value_ref);
            ctx.set_result_ref(value_ref);
            let mut abort_stack = Vec::new();
            value.emit_llvm((state.0, state.1), ctx, &mut abort_stack)?;
            function_call_abort_stack.extend(abort_stack);

            ctx.vrl_vec_insert().build_call(
                ctx.builder(),
                vec_ref,
                ctx.usize_type().const_int(index as _, false),
                value_ref,
            );
        }

        let set_result_block = ctx
            .context()
            .append_basic_block(function, "array_set_result");
        ctx.builder().build_unconditional_branch(set_result_block);
        ctx.builder().position_at_end(set_result_block);

        ctx.vrl_expression_array_set_result()
            .build_call(ctx.builder(), vec_ref, result_ref);

        ctx.builder().build_unconditional_branch(end_block);
        ctx.builder().position_at_end(end_block);

        ctx.set_result_ref(result_ref);

        Ok(())
    }
}

impl fmt::Display for Array {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let exprs = self
            .inner
            .iter()
            .map(Expr::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        write!(f, "[{}]", exprs)
    }
}

impl From<Vec<Expr>> for Array {
    fn from(inner: Vec<Expr>) -> Self {
        Self { inner }
    }
}

#[cfg(test)]
mod tests {
    use value::kind::Collection;

    use super::*;
    use crate::{expr, test_type_def, value::Kind, TypeDef};

    test_type_def![
        empty_array {
            expr: |_| expr!([]),
            want: TypeDef::array(Collection::empty()),
        }

        scalar_array {
            expr: |_| expr!([1, "foo", true]),
            want: TypeDef::array(BTreeMap::from([
                (0.into(), Kind::integer()),
                (1.into(), Kind::bytes()),
                (2.into(), Kind::boolean()),
            ])),
        }

        mixed_array {
            expr: |_| expr!([1, [true, "foo"], { "bar": null }]),
            want: TypeDef::array(BTreeMap::from([
                (0.into(), Kind::integer()),
                (1.into(), Kind::array(BTreeMap::from([
                    (0.into(), Kind::boolean()),
                    (1.into(), Kind::bytes()),
                ]))),
                (2.into(), Kind::object(BTreeMap::from([
                    ("bar".into(), Kind::null())
                ]))),
            ])),
        }
    ];
}
