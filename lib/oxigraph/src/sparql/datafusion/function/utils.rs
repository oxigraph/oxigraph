use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::{decode_term, encode_term};
use datafusion::arrow::array::{Array, ArrayIter, ArrayRef, BinaryArray, BooleanArray};
use datafusion::arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::common::{DataFusionError, Result, ScalarValue, downcast_value, internal_err};
use datafusion::logical_expr::expr::AggregateFunction;
use datafusion::logical_expr::function::{AccumulatorArgs, StateFieldsArgs};
use datafusion::logical_expr::{
    Accumulator, AggregateUDF, AggregateUDFImpl, Coercion, ColumnarValue, Expr, ScalarFunctionArgs,
    ScalarUDF, ScalarUDFImpl, Signature, TypeSignatureClass, Volatility,
};
use oxsdatatypes::Integer;
use spareval::{ExpressionTerm, QueryableDataset};
use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub fn term_function<const C: usize>(
    dataset: Arc<DatasetView<'static>>,
    inputs: [Expr; C],
    name: &'static str,
    eval: impl Fn([ExpressionTerm; C]) -> Option<ExpressionTerm> + Send + Sync + 'static,
    volatility: Volatility,
) -> Expr {
    ScalarUDF::new_from_impl(TermFunction {
        dataset,
        signature: Signature::coercible(
            vec![Coercion::new_exact(TypeSignatureClass::Binary); C],
            volatility,
        ),
        name,
        eval,
    })
    .call(inputs.to_vec())
}

struct TermFunction<F, const C: usize> {
    dataset: Arc<DatasetView<'static>>,
    signature: Signature,
    name: &'static str,
    eval: F,
}

impl<F, const C: usize> fmt::Debug for TermFunction<F, C> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.name).finish_non_exhaustive()
    }
}

impl<F, const C: usize> PartialEq for TermFunction<F, C> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<F, const C: usize> Eq for TermFunction<F, C> {}

impl<F, const C: usize> Hash for TermFunction<F, C> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<F: Fn([ExpressionTerm; C]) -> Option<ExpressionTerm> + Send + Sync + 'static, const C: usize>
    ScalarUDFImpl for TermFunction<F, C>
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

    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
        Ok(if arg_types.iter().any(DataType::is_null) {
            DataType::Null
        } else {
            DataType::Binary
        })
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let Some(args) = FunctionArgs::new(&args.args, &self.dataset, args.number_rows)? else {
            return Ok(ColumnarValue::Scalar(ScalarValue::Null));
        };
        let result: ArrayRef = Arc::new(
            args.map(|input| {
                let Some(input) = input? else {
                    return Ok(None);
                };
                let Some(result) = (self.eval)(input) else {
                    return Ok(None);
                };
                Ok(Some(encode_term(
                    &self.dataset.internalize_expression_term(result)?,
                )))
            })
            .collect::<Result<BinaryArray>>()?,
        );
        Ok(result.into())
    }
}

pub fn boolean_function<const C: usize>(
    dataset: Arc<DatasetView<'static>>,
    inputs: [Expr; C],
    name: &'static str,
    eval: impl Fn([ExpressionTerm; C]) -> Option<bool> + Send + Sync + 'static,
    volatility: Volatility,
) -> Expr {
    ScalarUDF::new_from_impl(BooleanFunction {
        dataset,
        signature: Signature::coercible(
            vec![Coercion::new_exact(TypeSignatureClass::Binary); C],
            volatility,
        ),
        name,
        eval,
    })
    .call(inputs.into())
}

struct BooleanFunction<F, const C: usize> {
    dataset: Arc<DatasetView<'static>>,
    signature: Signature,
    name: &'static str,
    eval: F,
}

impl<F, const C: usize> fmt::Debug for BooleanFunction<F, C> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.name).finish_non_exhaustive()
    }
}

impl<F, const C: usize> PartialEq for BooleanFunction<F, C> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<F, const C: usize> Eq for BooleanFunction<F, C> {}

impl<F, const C: usize> Hash for BooleanFunction<F, C> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<F: Fn([ExpressionTerm; C]) -> Option<bool> + Send + Sync + 'static, const C: usize>
    ScalarUDFImpl for BooleanFunction<F, C>
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

    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
        Ok(if arg_types.iter().any(DataType::is_null) {
            DataType::Null
        } else {
            DataType::Boolean
        })
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let Some(args) = FunctionArgs::new(&args.args, &self.dataset, args.number_rows)? else {
            return Ok(ColumnarValue::Scalar(ScalarValue::Null));
        };
        let result: ArrayRef = Arc::new(
            args.map(|input| {
                let Some(input) = input? else {
                    return Ok(None);
                };
                Ok((self.eval)(input))
            })
            .collect::<Result<BooleanArray>>()?,
        );
        Ok(result.into())
    }
}

enum FunctionArg<'a> {
    Single(ExpressionTerm),
    Array(ArrayIter<&'a BinaryArray>),
}

struct FunctionArgs<'a, const C: usize> {
    args: Vec<FunctionArg<'a>>,
    dataset: &'a DatasetView<'static>,
    len: usize,
}

impl<'a, const C: usize> FunctionArgs<'a, C> {
    fn new(
        args: &'a [ColumnarValue],
        dataset: &'a DatasetView<'static>,
        len: usize,
    ) -> Result<Option<Self>> {
        debug_assert!(args.len() == C, "Wrong number of arguments for function");
        Ok(args
            .iter()
            .map(|a| match a {
                ColumnarValue::Scalar(s) => match s {
                    ScalarValue::Binary(v) => {
                        let Some(v) = v else {
                            return Ok(None);
                        };
                        Ok(Some(FunctionArg::Single(
                            dataset.externalize_expression_term(decode_term(v)?)?,
                        )))
                    }
                    ScalarValue::Null => Ok(None),
                    _ => internal_err!("Unexpected function input datatype type {}", s.data_type()),
                },
                ColumnarValue::Array(a) => match a.data_type() {
                    DataType::Binary => Ok(Some(FunctionArg::Array(
                        downcast_value!(a, BinaryArray).iter(),
                    ))),
                    DataType::Null => Ok(None),
                    _ => internal_err!("Unexpected function input datatype type {}", a.data_type()),
                },
            })
            .collect::<Result<Option<Vec<FunctionArg<'_>>>>>()?
            .map(|args| Self { args, dataset, len }))
    }

    fn compute_next(&mut self) -> Result<Option<[ExpressionTerm; C]>> {
        let mut result = [const { ExpressionTerm::IntegerLiteral(Integer::MAX) }; C]; // TODO: figure out better default
        for (i, arg) in self.args.iter_mut().enumerate() {
            result[i] = match arg {
                FunctionArg::Single(v) => v.clone(),
                FunctionArg::Array(v) => {
                    let Some(v) = v.next().ok_or_else(|| {
                        DataFusionError::Internal("Unexpected end of array".into())
                    })?
                    else {
                        return Ok(None);
                    };
                    self.dataset.externalize_expression_term(decode_term(v)?)?
                }
            };
        }
        Ok(Some(result))
    }
}

impl<const C: usize> Iterator for FunctionArgs<'_, C> {
    type Item = Result<Option<[ExpressionTerm; C]>>;

    fn next(&mut self) -> Option<Result<Option<[ExpressionTerm; C]>>> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.compute_next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<const C: usize> ExactSizeIterator for FunctionArgs<'_, C> {}

pub fn term_aggregate_function<T: TermAccumulator>(
    dataset: Arc<DatasetView<'static>>,
    input: Expr,
    distinct: bool,
    name: &'static str,
    eval: impl Fn() -> T + Send + Sync + 'static,
    volatility: Volatility,
) -> Expr {
    Expr::AggregateFunction(AggregateFunction::new_udf(
        Arc::new(AggregateUDF::new_from_impl(TermAggregateFunction {
            dataset,
            signature: Signature::coercible(
                vec![Coercion::new_exact(TypeSignatureClass::Binary)],
                volatility,
            ),
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

struct TermAggregateFunction<F> {
    dataset: Arc<DatasetView<'static>>,
    signature: Signature,
    name: &'static str,
    eval: F,
}

impl<F> fmt::Debug for TermAggregateFunction<F> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.name).finish_non_exhaustive()
    }
}

impl<F> PartialEq for TermAggregateFunction<F> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<F> Eq for TermAggregateFunction<F> {}

impl<F> Hash for TermAggregateFunction<F> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<F: Fn() -> A + Send + Sync + 'static, A: TermAccumulator> AggregateUDFImpl
    for TermAggregateFunction<F>
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
        Ok(DataType::Binary)
    }

    fn state_fields(&self, _: StateFieldsArgs<'_>) -> Result<Vec<FieldRef>> {
        Ok(A::STATE_COLUMNS
            .iter()
            .map(|name| Arc::new(Field::new(*name, DataType::Binary, true)))
            .collect())
    }

    fn accumulator(&self, _acc_args: AccumulatorArgs<'_>) -> Result<Box<dyn Accumulator>> {
        Ok(Box::new(TermAccumulatorImpl {
            name: self.name,
            dataset: Arc::clone(&self.dataset),
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

struct TermAccumulatorImpl<A> {
    name: &'static str,
    dataset: Arc<DatasetView<'static>>,
    eval: Option<A>,
}

impl<A> fmt::Debug for TermAccumulatorImpl<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.name).finish_non_exhaustive()
    }
}

impl<A: TermAccumulator> Accumulator for TermAccumulatorImpl<A> {
    fn update_batch(&mut self, values: &[ArrayRef]) -> Result<()> {
        let Some(eval) = &mut self.eval else {
            return Ok(());
        };
        let input = &values[0];
        match input.data_type() {
            DataType::Binary => {
                for arg in downcast_value!(input, BinaryArray) {
                    let Some(arg) = arg else {
                        self.eval = None;
                        return Ok(());
                    };
                    eval.update(
                        self.dataset
                            .externalize_expression_term(decode_term(arg)?)?,
                    );
                }
                Ok(())
            }
            DataType::Null => {
                if !input.is_empty() {
                    self.eval = None;
                }
                Ok(())
            }
            _ => internal_err!(
                "Unexpected function input datatype type {}",
                input.data_type()
            ),
        }
    }

    fn evaluate(&mut self) -> Result<ScalarValue> {
        let Some(eval) = self.eval.take() else {
            return Ok(ScalarValue::Binary(None));
        };
        let Some(result) = eval.evaluate()? else {
            return Ok(ScalarValue::Binary(None));
        };
        Ok(ScalarValue::Binary(Some(encode_term(
            &self.dataset.internalize_expression_term(result)?,
        ))))
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
                Ok(ScalarValue::Binary(if let Some(t) = t {
                    Some(encode_term(&self.dataset.internalize_expression_term(t)?))
                } else {
                    None
                }))
            })
            .collect()
    }

    fn merge_batch(&mut self, states: &[ArrayRef]) -> Result<()> {
        let Some(eval) = &mut self.eval else {
            return Ok(());
        };
        if states.is_empty() {
            self.eval = None;
            return Ok(());
        }
        let mut iters = states
            .iter()
            .map(|s| Ok(downcast_value!(s, BinaryArray).iter()))
            .collect::<Result<Vec<_>>>()?;
        'outer: loop {
            let mut args = Vec::new();
            for iter in &mut iters {
                let Some(t) = iter.next() else {
                    break 'outer;
                };
                args.push(if let Some(t) = t {
                    Some(self.dataset.externalize_expression_term(decode_term(t)?)?)
                } else {
                    None
                })
            }
            eval.merge(args)?;
        }
        Ok(())
    }
}
