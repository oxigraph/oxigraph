use model::*;
use sparql::algebra::TermOrVariable;
use sparql::algebra::Variable;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Binding(BTreeMap<Variable, Term>);

impl Binding {
    pub fn insert(&mut self, var: Variable, value: Term) {
        self.0.insert(var, value);
    }

    pub fn get<'a>(&'a self, key: &'a Variable) -> Option<&'a Term> {
        self.0.get(key)
    }

    pub fn get_or_constant<'a>(&'a self, key: &'a TermOrVariable) -> Option<Term> {
        match key {
            TermOrVariable::NamedNode(node) => Some(node.clone().into()),
            TermOrVariable::Literal(literal) => Some(literal.clone().into()),
            TermOrVariable::Variable(v) => self.get(v).cloned(),
        }
    }

    pub fn iter(&self) -> <&BTreeMap<Variable, Term> as IntoIterator>::IntoIter {
        self.0.iter()
    }
}

impl Default for Binding {
    fn default() -> Self {
        Binding(BTreeMap::default())
    }
}

impl IntoIterator for Binding {
    type Item = (Variable, Term);
    type IntoIter = <BTreeMap<Variable, Term> as IntoIterator>::IntoIter;

    fn into_iter(self) -> <BTreeMap<Variable, Term> as IntoIterator>::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Binding {
    type Item = (&'a Variable, &'a Term);
    type IntoIter = <&'a BTreeMap<Variable, Term> as IntoIterator>::IntoIter;

    fn into_iter(self) -> <&'a BTreeMap<Variable, Term> as IntoIterator>::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{")?;
        for (var, val) in self {
            write!(f, " {} → {} ", var, val)?;
        }
        write!(f, "}}")
    }
}
