//! Definition of [`QuerySolution`] structure and associated utility constructions.

use oxrdf::{Term, Variable, VariableRef};
use std::iter::Zip;
use std::ops::Index;
use std::rc::Rc;

/// Tuple associating variables and terms that are the result of a SPARQL query.
///
/// It is the equivalent of a row in SQL.
///
/// ```
/// use sparesults::QuerySolution;
/// use oxrdf::{Variable, Literal};
///
/// let solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![Some(Literal::from(1).into()), None]));
/// assert_eq!(solution.get("foo"), Some(&Literal::from(1).into())); // Get the value of the variable ?foo if it exists (here yes).
/// assert_eq!(solution.get(1), None); // Get the value of the second column if it exists (here no).
/// ```
pub struct QuerySolution {
    variables: Rc<Vec<Variable>>,
    values: Vec<Option<Term>>,
}

impl QuerySolution {
    /// Returns a value for a given position in the tuple ([`usize`](std::usize)) or a given variable name ([`&str`](std::str), [`Variable`] or [`VariableRef`]).
    ///
    /// ```
    /// use sparesults::QuerySolution;
    /// use oxrdf::{Variable, Literal};
    ///
    /// let solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![Some(Literal::from(1).into()), None]));
    /// assert_eq!(solution.get("foo"), Some(&Literal::from(1).into())); // Get the value of the variable ?foo if it exists (here yes).
    /// assert_eq!(solution.get(1), None); // Get the value of the second column if it exists (here no).
    /// ```
    #[inline]
    pub fn get(&self, index: impl VariableSolutionIndex) -> Option<&Term> {
        self.values
            .get(index.index(self)?)
            .and_then(std::option::Option::as_ref)
    }

    /// The number of variables which could be bound.
    ///
    /// It is also the number of columns in the solutions table.
    ///
    /// ```
    /// use sparesults::QuerySolution;
    /// use oxrdf::{Variable, Literal};
    ///
    /// let solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![Some(Literal::from(1).into()), None]));
    /// assert_eq!(solution.len(), 2);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Is there any variable bound in the table?
    ///
    /// ```
    /// use sparesults::QuerySolution;
    /// use oxrdf::{Variable, Literal};
    ///
    /// let solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![Some(Literal::from(1).into()), None]));
    /// assert!(!solution.is_empty());
    ///
    /// let empty_solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![None, None]));
    /// assert!(empty_solution.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.iter().all(|v| v.is_none())
    }

    /// Returns an iterator over bound variables.
    ///
    /// ```
    /// use sparesults::QuerySolution;
    /// use oxrdf::{Variable, Literal};
    ///
    /// let solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![Some(Literal::from(1).into()), None]));
    /// assert_eq!(solution.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from(1).into())]);
    /// ```
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&Variable, &Term)> {
        self.into_iter()
    }

    /// Returns the ordered slice of variable values.
    ///
    /// ```
    /// use sparesults::QuerySolution;
    /// use oxrdf::{Variable, Literal};
    ///
    /// let solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![Some(Literal::from(1).into()), None]));
    /// assert_eq!(solution.values(), &[Some(Literal::from(1).into()), None]);
    /// ```
    #[inline]
    pub fn values(&self) -> &[Option<Term>] {
        &self.values
    }

    /// Returns the ordered slice of the solution variables, bound or not.
    ///
    /// ```
    /// use sparesults::QuerySolution;
    /// use oxrdf::{Variable, Literal};
    ///
    /// let solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![Some(Literal::from(1).into()), None]));
    /// assert_eq!(solution.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<V: Into<Rc<Vec<Variable>>>, S: Into<Vec<Option<Term>>>> From<(V, S)> for QuerySolution {
    #[inline]
    fn from((v, s): (V, S)) -> Self {
        QuerySolution {
            variables: v.into(),
            values: s.into(),
        }
    }
}

impl<'a> IntoIterator for &'a QuerySolution {
    type Item = (&'a Variable, &'a Term);
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Iter<'a> {
        Iter {
            inner: self.variables.iter().zip(&self.values),
        }
    }
}

impl Index<usize> for QuerySolution {
    type Output = Term;

    #[inline]
    fn index(&self, index: usize) -> &Term {
        self.get(index)
            .unwrap_or_else(|| panic!("The column {} is not set in this solution", index))
    }
}

impl Index<&str> for QuerySolution {
    type Output = Term;

    #[inline]
    fn index(&self, index: &str) -> &Term {
        self.get(index)
            .unwrap_or_else(|| panic!("The variable ?{} is not set in this solution", index))
    }
}

impl Index<VariableRef<'_>> for QuerySolution {
    type Output = Term;

    #[inline]
    fn index(&self, index: VariableRef<'_>) -> &Term {
        self.get(index)
            .unwrap_or_else(|| panic!("The variable {} is not set in this solution", index))
    }
}
impl Index<Variable> for QuerySolution {
    type Output = Term;

    #[inline]
    fn index(&self, index: Variable) -> &Term {
        self.index(index.as_ref())
    }
}

impl Index<&Variable> for QuerySolution {
    type Output = Term;

    #[inline]
    fn index(&self, index: &Variable) -> &Term {
        self.index(index.as_ref())
    }
}

/// An iterator over [`QuerySolution`] bound variables.
///
/// ```
/// use sparesults::QuerySolution;
/// use oxrdf::{Variable, Literal};
///
/// let solution = QuerySolution::from((vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")], vec![Some(Literal::from(1).into()), None]));
/// assert_eq!(solution.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from(1).into())]);
/// ```
pub struct Iter<'a> {
    inner: Zip<std::slice::Iter<'a, Variable>, std::slice::Iter<'a, Option<Term>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Variable, &'a Term);

    #[inline]
    fn next(&mut self) -> Option<(&'a Variable, &'a Term)> {
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
