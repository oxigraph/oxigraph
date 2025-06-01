//! Definition of [`QuerySolution`] structure and associated utility constructions.

use oxrdf::{Term, Variable, VariableRef};
use std::fmt;
use std::iter::Zip;
use std::ops::Index;
use std::sync::Arc;

/// Tuple associating variables and terms that are the result of a SPARQL query.
///
/// It is the equivalent of a row in SQL.
///
/// ```
/// use sparesults::QuerySolution;
/// use oxrdf::{Variable, Literal};
///
/// let solution = QuerySolution::from((vec![Variable::new("foo")?, Variable::new("bar")?], vec![Some(Literal::from(1).into()), None]));
/// assert_eq!(solution.get("foo"), Some(&Literal::from(1).into())); // Get the value of the variable ?foo if it exists (here yes).
/// assert_eq!(solution.get(1), None); // Get the value of the second column if it exists (here no).
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QuerySolution {
    variables: Arc<[Variable]>,
    values: Vec<Option<Term>>,
}

impl QuerySolution {
    /// Returns a value for a given position in the tuple ([`usize`](std::usize)) or a given variable name ([`&str`](std::str), [`Variable`] or [`VariableRef`]).
    ///
    /// ```
    /// use sparesults::QuerySolution;
    /// use oxrdf::{Variable, Literal};
    ///
    /// let solution = QuerySolution::from((vec![Variable::new("foo")?, Variable::new("bar")?], vec![Some(Literal::from(1).into()), None]));
    /// assert_eq!(solution.get("foo"), Some(&Literal::from(1).into())); // Get the value of the variable ?foo if it exists (here yes).
    /// assert_eq!(solution.get(1), None); // Get the value of the second column if it exists (here no).
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn get(&self, index: impl VariableSolutionIndex) -> Option<&Term> {
        self.values.get(index.index(self)?).and_then(Option::as_ref)
    }

    /// The number of variables which could be bound.
    ///
    /// It is also the number of columns in the solutions table.
    ///
    /// ```
    /// use oxrdf::{Literal, Variable};
    /// use sparesults::QuerySolution;
    ///
    /// let solution = QuerySolution::from((
    ///     vec![Variable::new("foo")?, Variable::new("bar")?],
    ///     vec![Some(Literal::from(1).into()), None],
    /// ));
    /// assert_eq!(solution.len(), 2);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Is there any variable bound in the table?
    ///
    /// ```
    /// use oxrdf::{Literal, Variable};
    /// use sparesults::QuerySolution;
    ///
    /// let solution = QuerySolution::from((
    ///     vec![Variable::new("foo")?, Variable::new("bar")?],
    ///     vec![Some(Literal::from(1).into()), None],
    /// ));
    /// assert!(!solution.is_empty());
    ///
    /// let empty_solution = QuerySolution::from((
    ///     vec![Variable::new("foo")?, Variable::new("bar")?],
    ///     vec![None, None],
    /// ));
    /// assert!(empty_solution.is_empty());
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.iter().all(Option::is_none)
    }

    /// Returns an iterator over bound variables.
    ///
    /// ```
    /// use oxrdf::{Literal, Variable};
    /// use sparesults::QuerySolution;
    ///
    /// let solution = QuerySolution::from((
    ///     vec![Variable::new("foo")?, Variable::new("bar")?],
    ///     vec![Some(Literal::from(1).into()), None],
    /// ));
    /// assert_eq!(
    ///     solution.iter().collect::<Vec<_>>(),
    ///     vec![(&Variable::new("foo")?, &Literal::from(1).into())]
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&Variable, &Term)> {
        self.into_iter()
    }

    /// Returns the ordered slice of variable values.
    ///
    /// ```
    /// use oxrdf::{Literal, Variable};
    /// use sparesults::QuerySolution;
    ///
    /// let solution = QuerySolution::from((
    ///     vec![Variable::new("foo")?, Variable::new("bar")?],
    ///     vec![Some(Literal::from(1).into()), None],
    /// ));
    /// assert_eq!(solution.values(), &[Some(Literal::from(1).into()), None]);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn values(&self) -> &[Option<Term>] {
        &self.values
    }

    /// Returns the ordered slice of the solution variables, bound or not.
    ///
    /// ```
    /// use oxrdf::{Literal, Variable};
    /// use sparesults::QuerySolution;
    ///
    /// let solution = QuerySolution::from((
    ///     vec![Variable::new("foo")?, Variable::new("bar")?],
    ///     vec![Some(Literal::from(1).into()), None],
    /// ));
    /// assert_eq!(
    ///     solution.variables(),
    ///     &[Variable::new("foo")?, Variable::new("bar")?]
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<V: Into<Arc<[Variable]>>, S: Into<Vec<Option<Term>>>> From<(V, S)> for QuerySolution {
    #[inline]
    fn from((v, s): (V, S)) -> Self {
        Self {
            variables: v.into(),
            values: s.into(),
        }
    }
}

impl<'a> IntoIterator for &'a QuerySolution {
    type Item = (&'a Variable, &'a Term);
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            inner: self.variables.iter().zip(&self.values),
        }
    }
}

impl Index<usize> for QuerySolution {
    type Output = Term;

    #[expect(clippy::panic)]
    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("The column {index} is not set in this solution"))
    }
}

impl Index<&str> for QuerySolution {
    type Output = Term;

    #[expect(clippy::panic)]
    #[inline]
    fn index(&self, index: &str) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("The variable ?{index} is not set in this solution"))
    }
}

impl Index<VariableRef<'_>> for QuerySolution {
    type Output = Term;

    #[expect(clippy::panic)]
    #[inline]
    fn index(&self, index: VariableRef<'_>) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("The variable {index} is not set in this solution"))
    }
}
impl Index<Variable> for QuerySolution {
    type Output = Term;

    #[inline]
    fn index(&self, index: Variable) -> &Self::Output {
        self.index(index.as_ref())
    }
}

impl Index<&Variable> for QuerySolution {
    type Output = Term;

    #[inline]
    fn index(&self, index: &Variable) -> &Self::Output {
        self.index(index.as_ref())
    }
}

impl PartialEq for QuerySolution {
    fn eq(&self, other: &Self) -> bool {
        for (k, v) in self.iter() {
            if other.get(k) != Some(v) {
                return false;
            }
        }
        for (k, v) in other.iter() {
            if self.get(k) != Some(v) {
                return false;
            }
        }
        true
    }
}

impl Eq for QuerySolution {}

impl fmt::Debug for QuerySolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// An iterator over [`QuerySolution`] bound variables.
///
/// ```
/// use oxrdf::{Literal, Variable};
/// use sparesults::QuerySolution;
///
/// let solution = QuerySolution::from((
///     vec![Variable::new("foo")?, Variable::new("bar")?],
///     vec![Some(Literal::from(1).into()), None],
/// ));
/// assert_eq!(
///     solution.iter().collect::<Vec<_>>(),
///     vec![(&Variable::new("foo")?, &Literal::from(1).into())]
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct Iter<'a> {
    inner: Zip<std::slice::Iter<'a, Variable>, std::slice::Iter<'a, Option<Term>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Variable, &'a Term);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for (variable, value) in &mut self.inner {
            if let Some(value) = value {
                return Some((variable, value));
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.inner.size_hint().1)
    }
}

/// A utility trait to get values for a given variable or tuple position.
///
/// See [`QuerySolution::get`].
pub trait VariableSolutionIndex {
    fn index(self, solution: &QuerySolution) -> Option<usize>;
}

impl VariableSolutionIndex for usize {
    #[inline]
    fn index(self, _: &QuerySolution) -> Option<usize> {
        Some(self)
    }
}

impl VariableSolutionIndex for &str {
    #[inline]
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        solution.variables.iter().position(|v| v.as_str() == self)
    }
}

impl VariableSolutionIndex for VariableRef<'_> {
    #[inline]
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        solution.variables.iter().position(|v| *v == self)
    }
}

impl VariableSolutionIndex for &Variable {
    #[inline]
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        self.as_ref().index(solution)
    }
}

impl VariableSolutionIndex for Variable {
    #[inline]
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        self.as_ref().index(solution)
    }
}
