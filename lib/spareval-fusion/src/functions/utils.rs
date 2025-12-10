use crate::dataset::ExpressionTermEncoder;
use datafusion::arrow::array::{Array, ArrayRef, BooleanArray};
use datafusion::arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::common::types::NativeType;
use datafusion::common::{Result, ScalarValue};
use datafusion::logical_expr::expr::AggregateFunction;
use datafusion::logical_expr::function::{AccumulatorArgs, StateFieldsArgs};
use datafusion::logical_expr::{
    Accumulator, AggregateUDF, AggregateUDFImpl, Coercion, ColumnarValue, Expr, ScalarFunctionArgs,
    ScalarUDF, ScalarUDFImpl, Signature, TypeSignatureClass, Volatility,
};
use oxsdatatypes::Integer;
use spareval::ExpressionTerm;
use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::vec::IntoIter;

pub fn term_function<const C: usize>(
    encoder: impl ExpressionTermEncoder,
    inputs: [Expr; C],
    name: &'static str,
    eval: impl Fn([ExpressionTerm; C]) -> Option<ExpressionTerm> + Send + Sync + 'static,
    volatility: Volatility,
) -> Expr {
    let signature = signature(&encoder, C, volatility);
    ScalarUDF::new_from_impl(TermFunction {
        encoder,
        signature,
        name,
        eval,
    })
    .call(inputs.to_vec())
}

struct TermFunction<E, F, const C: usize> {
    encoder: E,
    signature: Signature,
    name: &'static str,
    eval: F,
}

impl<E, F, const C: usize> fmt::Debug for TermFunction<E, F, C> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.name).finish_non_exhaustive()
    }
}

impl<E, F, const C: usize> PartialEq for TermFunction<E, F, C> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<E, F, const C: usize> Eq for TermFunction<E, F, C> {}

impl<E, F, const C: usize> Hash for TermFunction<E, F, C> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<
    E: ExpressionTermEncoder,
    F: Fn([ExpressionTerm; C]) -> Option<ExpressionTerm> + Send + Sync + 'static,
    const C: usize,
> ScalarUDFImpl for TermFunction<E, F, C>
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        self.name
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> Result<DataType> {
        Ok(self.encoder.internal_type().clone())
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let Some(args) = FunctionArgs::new(args.args, &self.encoder, args.number_rows)? else {
            return Ok(ScalarValue::try_new_null(self.encoder.internal_type())?.into());
        }; // TODO: rewrite FunctionArgs to be more efficient
        let result: ArrayRef = self
            .encoder
            .internalize_expression_terms(args.into_iter().map(|input| (self.eval)(input?)))?;
        Ok(result.into())
    }
}

pub fn boolean_function<const C: usize>(
    encoder: impl ExpressionTermEncoder,
    inputs: [Expr; C],
    name: &'static str,
    eval: impl Fn([ExpressionTerm; C]) -> Option<bool> + Send + Sync + 'static,
    volatility: Volatility,
) -> Expr {
    let signature = signature(&encoder, C, volatility);
    ScalarUDF::new_from_impl(BooleanFunction {
        encoder,
        signature,
        name,
        eval,
    })
    .call(inputs.into())
}

struct BooleanFunction<E, F, const C: usize> {
    encoder: E,
    signature: Signature,
    name: &'static str,
    eval: F,
}

impl<E, F, const C: usize> fmt::Debug for BooleanFunction<E, F, C> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.name).finish_non_exhaustive()
    }
}

impl<E, F, const C: usize> PartialEq for BooleanFunction<E, F, C> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<E, F, const C: usize> Eq for BooleanFunction<E, F, C> {}

impl<E, F, const C: usize> Hash for BooleanFunction<E, F, C> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<
    E: ExpressionTermEncoder,
    F: Fn([ExpressionTerm; C]) -> Option<bool> + Send + Sync + 'static,
    const C: usize,
> ScalarUDFImpl for BooleanFunction<E, F, C>
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        self.name
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> Result<DataType> {
        Ok(DataType::Boolean)
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let Some(args) = FunctionArgs::new(args.args, &self.encoder, args.number_rows)? else {
            return Ok(ScalarValue::Boolean(None).into());
        };
        let result: ArrayRef = Arc::new(
            args.map(|input| (self.eval)(input?))
                .collect::<BooleanArray>(),
        );
        Ok(result.into())
    }
}

enum FunctionArg {
    Single(ExpressionTerm),
    Array(IntoIter<Option<ExpressionTerm>>),
}

struct FunctionArgs<const C: usize> {
    args: Vec<FunctionArg>,
    i: usize,
    len: usize,
}

impl<const C: usize> FunctionArgs<C> {
    fn new(
        args: Vec<ColumnarValue>,
        encoder: &impl ExpressionTermEncoder,
        len: usize,
    ) -> Result<Option<Self>> {
        debug_assert!(args.len() == C, "Wrong number of arguments for function");
        Ok(args
            .into_iter()
            .map(|a| match a {
                ColumnarValue::Scalar(s) => Ok(encoder
                    .externalize_expression_term(s)?
                    .map(FunctionArg::Single)),
                ColumnarValue::Array(a) => {
                    if a.data_type().is_null() {
                        return Ok(None);
                    }
                    Ok(Some(FunctionArg::Array(
                        encoder
                            .externalize_expression_terms(a)?
                            .into_iter()
                            .collect::<Result<Vec<_>>>()?
                            .into_iter(),
                    )))
                }
            })
            .collect::<Result<Option<Vec<FunctionArg>>>>()?
            .map(|args| Self { args, len, i: 0 }))
    }

    fn compute_next(&mut self) -> Option<[ExpressionTerm; C]> {
        let mut result = [const { ExpressionTerm::IntegerLiteral(Integer::MAX) }; C]; // TODO: figure out better default
        for (i, arg) in self.args.iter_mut().enumerate() {
            result[i] = match arg {
                FunctionArg::Single(v) => v.clone(),
                FunctionArg::Array(v) => v.next()??,
            };
        }
        Some(result)
    }
}

impl<const C: usize> Iterator for FunctionArgs<C> {
    type Item = Option<[ExpressionTerm; C]>;

    fn next(&mut self) -> Option<Option<[ExpressionTerm; C]>> {
        if self.i == self.len {
            return None;
        }
        let result = Some(self.compute_next());
        self.i += 1;
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<const C: usize> ExactSizeIterator for FunctionArgs<C> {}

pub fn term_aggregate_function<T: TermAccumulator>(
    encoder: impl ExpressionTermEncoder,
    input: Expr,
    distinct: bool,
    name: &'static str,
    eval: impl Fn() -> T + Send + Sync + 'static,
    volatility: Volatility,
) -> Expr {
    let signature = signature(&encoder, 1, volatility);
    Expr::AggregateFunction(AggregateFunction::new_udf(
        Arc::new(AggregateUDF::new_from_impl(TermAggregateFunction {
            encoder,
            signature,
            name,
            eval,
        })),
        vec![input],
        distinct,
        None,
        Vec::new(),
        None,
    ))
}

struct TermAggregateFunction<E, F> {
    encoder: E,
    signature: Signature,
    name: &'static str,
    eval: F,
}

impl<E, F> fmt::Debug for TermAggregateFunction<E, F> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.name).finish_non_exhaustive()
    }
}

impl<E, F> PartialEq for TermAggregateFunction<E, F> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<E, F> Eq for TermAggregateFunction<E, F> {}

impl<E, F> Hash for TermAggregateFunction<E, F> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<E: ExpressionTermEncoder, F: Fn() -> A + Send + Sync + 'static, A: TermAccumulator>
    AggregateUDFImpl for TermAggregateFunction<E, F>
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        self.name
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> Result<DataType> {
        Ok(self.encoder.internal_type().clone())
    }

    fn state_fields(&self, _: StateFieldsArgs<'_>) -> Result<Vec<FieldRef>> {
        Ok(A::STATE_COLUMNS
            .iter()
            .map(|name| {
                Arc::new(Field::new(
                    *name,
                    self.encoder.internal_type().clone(),
                    true,
                ))
            })
            .collect())
    }

    fn accumulator(&self, _acc_args: AccumulatorArgs<'_>) -> Result<Box<dyn Accumulator>> {
        Ok(Box::new(TermAccumulatorImpl {
            name: self.name,
            encoder: self.encoder.clone(),
            eval: Some((self.eval)()),
        }))
    }
}

pub trait TermAccumulator: Send + Sync + 'static {
    const STATE_COLUMNS: &[&str];

    fn update(&mut self, term: ExpressionTerm);

    fn evaluate(self) -> Result<Option<ExpressionTerm>>;

    fn state(self) -> Vec<Option<ExpressionTerm>>;

    fn merge(&mut self, state: Vec<Option<ExpressionTerm>>) -> Result<()>;
}

struct TermAccumulatorImpl<E, A> {
    name: &'static str,
    encoder: E,
    eval: Option<A>,
}

impl<E, A> fmt::Debug for TermAccumulatorImpl<E, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.name).finish_non_exhaustive()
    }
}

impl<E: ExpressionTermEncoder, A: TermAccumulator> Accumulator for TermAccumulatorImpl<E, A> {
    fn update_batch(&mut self, values: &[ArrayRef]) -> Result<()> {
        let Some(eval) = &mut self.eval else {
            return Ok(());
        };
        for arg in self
            .encoder
            .externalize_expression_terms(Arc::clone(&values[0]))?
        {
            let Some(arg) = arg? else {
                self.eval = None;
                return Ok(());
            };
            eval.update(arg);
        }
        Ok(())
    }

    fn evaluate(&mut self) -> Result<ScalarValue> {
        let Some(eval) = self.eval.take() else {
            return ScalarValue::try_new_null(self.encoder.internal_type());
        };
        let Some(result) = eval.evaluate()? else {
            return ScalarValue::try_new_null(self.encoder.internal_type());
        };
        self.encoder.internalize_expression_term(result)
    }

    fn size(&self) -> usize {
        size_of_val(self)
    }

    fn state(&mut self) -> Result<Vec<ScalarValue>> {
        let Some(eval) = self.eval.take() else {
            return Ok(Vec::new());
        };
        eval.state()
            .into_iter()
            .map(|t| {
                if let Some(t) = t {
                    self.encoder.internalize_expression_term(t)
                } else {
                    ScalarValue::try_new_null(self.encoder.internal_type())
                }
            })
            .collect()
    }

    fn merge_batch(&mut self, states: &[ArrayRef]) -> Result<()> {
        let Some(eval) = &mut self.eval else {
            return Ok(());
        };
        let mut iters = states
            .iter()
            .map(|s| {
                Ok(self
                    .encoder
                    .externalize_expression_terms(Arc::clone(s))?
                    .into_iter())
            })
            .collect::<Result<Vec<_>>>()?;
        'outer: loop {
            let mut args = Vec::new();
            for iter in &mut iters {
                let Some(t) = iter.next() else {
                    break 'outer;
                };
                args.push(t?)
            }
            eval.merge(args)?;
        }
        Ok(())
    }
}

pub fn signature(
    encoder: &impl ExpressionTermEncoder,
    args_count: usize,
    volatility: Volatility,
) -> Signature {
    if args_count == 0 {
        return Signature::nullary(volatility);
    }
    Signature::coercible(
        vec![
            Coercion::new_exact(TypeSignatureClass::Native(Arc::new(NativeType::from(
                encoder.internal_type()
            ))));
            args_count
        ],
        volatility,
    )
}
