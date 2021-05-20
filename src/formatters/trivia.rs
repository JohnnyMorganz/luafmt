#[cfg(feature = "luau")]
use full_moon::ast::types::{IndexedTypeInfo, TypeAssertion, TypeInfo, TypeSpecifier};
use full_moon::ast::{
    punctuated::Punctuated, span::ContainedSpan, BinOp, Call, Expression, FunctionArgs,
    FunctionBody, FunctionCall, FunctionName, If, Index, LastStmt, MethodCall, Parameter, Prefix,
    Return, Suffix, TableConstructor, UnOp, Value, Var, VarExpression,
};
use full_moon::tokenizer::{Token, TokenReference};

/// Enum to determine how trivia should be added when using trivia formatter functions
#[derive(Clone, Debug)]
pub enum FormatTriviaType<'ast> {
    /// Trivia will be added to the end of the current trivia
    Append(Vec<Token<'ast>>),
    /// The current trivia will be replaced with the new trivia
    Replace(Vec<Token<'ast>>),
    /// Trivia will not be changed
    NoChange,
}

/// Strips all leading and trailing trivia from a specific node.
/// This is useful if we need to use the node to calculate sizing, whilst we do not want trivia included
pub fn strip_trivia<'ast, T>(item: &T) -> T
where
    T: UpdateLeadingTrivia<'ast> + UpdateTrailingTrivia<'ast>,
{
    item.update_leading_trivia(FormatTriviaType::Replace(vec![]))
        .update_trailing_trivia(FormatTriviaType::Replace(vec![]))
}

pub fn strip_leading_trivia<'ast, T>(item: &T) -> T
where
    T: UpdateLeadingTrivia<'ast>,
{
    item.update_leading_trivia(FormatTriviaType::Replace(vec![]))
}

pub fn strip_trailing_trivia<'ast, T>(item: &T) -> T
where
    T: UpdateTrailingTrivia<'ast>,
{
    item.update_trailing_trivia(FormatTriviaType::Replace(vec![]))
}

pub trait UpdateLeadingTrivia<'ast> {
    fn update_leading_trivia(&self, leading_trivia: FormatTriviaType<'ast>) -> Self;
}

pub trait UpdateTrailingTrivia<'ast> {
    fn update_trailing_trivia(&self, trailing_trivia: FormatTriviaType<'ast>) -> Self;
}

pub trait UpdateTrivia<'ast> {
    fn update_trivia(
        &self,
        leading_trivia: FormatTriviaType<'ast>,
        trailing_trivia: FormatTriviaType<'ast>,
    ) -> Self;
}

impl<'ast, T> UpdateLeadingTrivia<'ast> for T
where
    T: UpdateTrivia<'ast>,
{
    fn update_leading_trivia(&self, leading_trivia: FormatTriviaType<'ast>) -> Self
    where
        Self: std::marker::Sized,
    {
        self.update_trivia(leading_trivia, FormatTriviaType::NoChange)
    }
}

impl<'ast, T> UpdateTrailingTrivia<'ast> for T
where
    T: UpdateTrivia<'ast>,
{
    fn update_trailing_trivia(&self, trailing_trivia: FormatTriviaType<'ast>) -> Self
    where
        Self: std::marker::Sized,
    {
        self.update_trivia(FormatTriviaType::NoChange, trailing_trivia)
    }
}

impl<'ast> UpdateTrivia<'ast> for TokenReference<'ast> {
    fn update_trivia(
        &self,
        leading_trivia: FormatTriviaType<'ast>,
        trailing_trivia: FormatTriviaType<'ast>,
    ) -> Self {
        let added_leading_trivia = match leading_trivia {
            FormatTriviaType::Append(trivia) => {
                let mut current: Vec<Token> = self.leading_trivia().map(|x| x.to_owned()).collect();
                current.extend(trivia);
                current
            }
            FormatTriviaType::Replace(trivia) => trivia,
            FormatTriviaType::NoChange => self.leading_trivia().map(|x| x.to_owned()).collect(),
        };
        let added_trailing_trivia = match trailing_trivia {
            FormatTriviaType::Append(trivia) => {
                let mut current: Vec<Token> =
                    self.trailing_trivia().map(|x| x.to_owned()).collect();
                current.extend(trivia);
                current
            }
            FormatTriviaType::Replace(trivia) => trivia,
            FormatTriviaType::NoChange => self.trailing_trivia().map(|x| x.to_owned()).collect(),
        };
        TokenReference::new(
            added_leading_trivia,
            self.token().to_owned(),
            added_trailing_trivia,
        )
    }
}

macro_rules! define_update_trivia {
    ($node:ident, |$self:ident, $leading_trivia:ident, $trailing_trivia:ident| $body:expr) => {
        define_update_trivia! {$node, |$self:&$node<'ast>, $leading_trivia: FormatTriviaType<'ast>, $trailing_trivia: FormatTriviaType<'ast>| $body}
    };
    ($node:ident, $body:expr) => {
        impl<'ast> UpdateTrivia<'ast> for $node<'ast> {
            fn update_trivia(&self, leading_trivia: FormatTriviaType<'ast>, trailing_trivia: FormatTriviaType<'ast>) -> Self {
                $body(&self, leading_trivia, trailing_trivia)
            }
        }
    };
}

macro_rules! define_update_leading_trivia {
    ($node:ident, |$self:ident, $leading_trivia:ident| $body:expr) => {
        define_update_leading_trivia! {$node, |$self:&$node<'ast>, $leading_trivia: FormatTriviaType<'ast>| $body}
    };
    ($node:ident, $body:expr) => {
        impl<'ast> UpdateLeadingTrivia<'ast> for $node<'ast> {
            fn update_leading_trivia(&self, leading_trivia: FormatTriviaType<'ast>) -> Self {
                $body(&self, leading_trivia)
            }
        }
    };
}

macro_rules! define_update_trailing_trivia {
    ($node:ident, |$self:ident, $trailing_trivia:ident| $body:expr) => {
        define_update_trailing_trivia! {$node, |$self:&$node<'ast>, $trailing_trivia: FormatTriviaType<'ast>| $body}
    };
    ($node:ident, $body:expr) => {
        impl<'ast> UpdateTrailingTrivia<'ast> for $node<'ast> {
            fn update_trailing_trivia(&self, trailing_trivia: FormatTriviaType<'ast>) -> Self {
                $body(&self, trailing_trivia)
            }
        }
    };
}

macro_rules! binop_trivia {
    ($enum:ident, $value:ident, $leading_trivia:ident, $trailing_trivia:ident, { $($operator:ident,)+ }) => {
        match $value {
            $(
                $enum::$operator(token) => $enum::$operator(token.update_trivia($leading_trivia, $trailing_trivia)),
            )+
            other => panic!("unknown node {:?}", other),
        }
    };
}

define_update_trivia!(BinOp, |this, leading, trailing| {
    binop_trivia!(BinOp, this, leading, trailing, {
        And,
        Caret,
        GreaterThan,
        GreaterThanEqual,
        LessThan,
        LessThanEqual,
        Minus,
        Or,
        Percent,
        Plus,
        Slash,
        Star,
        TildeEqual,
        TwoDots,
        TwoEqual,
    })
});

define_update_trivia!(ContainedSpan, |this, leading, trailing| {
    let (start_token, end_token) = this.tokens();
    ContainedSpan::new(
        start_token.update_leading_trivia(leading),
        end_token.update_trailing_trivia(trailing),
    )
});

define_update_trivia!(Call, |this, leading, trailing| {
    match this {
        Call::AnonymousCall(function_args) => {
            Call::AnonymousCall(function_args.update_trivia(leading, trailing))
        }
        Call::MethodCall(method_call) => {
            Call::MethodCall(method_call.update_trivia(leading, trailing))
        }
        other => panic!("unknown node {:?}", other),
    }
});

define_update_leading_trivia!(Expression, |this, leading| {
    match this {
        Expression::Parentheses {
            contained,
            expression,
        } => Expression::Parentheses {
            contained: contained.update_leading_trivia(leading),
            expression: expression.to_owned(),
        },
        Expression::UnaryOperator { unop, expression } => Expression::UnaryOperator {
            unop: unop.update_leading_trivia(leading),
            expression: expression.to_owned(),
        },
        Expression::BinaryOperator { lhs, binop, rhs } => Expression::BinaryOperator {
            lhs: Box::new(lhs.update_leading_trivia(leading)),
            binop: binop.to_owned(),
            rhs: rhs.to_owned(),
        },
        Expression::Value {
            value,
            #[cfg(feature = "luau")]
            type_assertion,
        } => Expression::Value {
            value: Box::new(value.update_leading_trivia(leading)),
            #[cfg(feature = "luau")]
            type_assertion: type_assertion.to_owned(),
        },
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trailing_trivia!(Expression, |this, trailing| {
    match this {
        Expression::Value {
            value,
            #[cfg(feature = "luau")]
            type_assertion,
        } => {
            #[cfg(feature = "luau")]
            if let Some(as_assertion) = type_assertion {
                return Expression::Value {
                    value: value.to_owned(),
                    type_assertion: Some(as_assertion.update_trailing_trivia(trailing)),
                };
            }

            Expression::Value {
                value: Box::new(value.update_trailing_trivia(trailing)),
                #[cfg(feature = "luau")]
                type_assertion: type_assertion.to_owned(),
            }
        }

        // Add trailing trivia to the end of parentheses
        Expression::Parentheses {
            contained,
            expression,
        } => Expression::Parentheses {
            contained: contained.update_trailing_trivia(trailing),
            expression: expression.to_owned(),
        },

        // Keep recursing down until we find an Expression::Value
        Expression::UnaryOperator { unop, expression } => Expression::UnaryOperator {
            unop: unop.to_owned(),
            expression: Box::new(expression.update_trailing_trivia(trailing)),
        },

        Expression::BinaryOperator { lhs, binop, rhs } => Expression::BinaryOperator {
            lhs: lhs.to_owned(),
            binop: binop.to_owned(),
            rhs: Box::new(rhs.update_trailing_trivia(trailing)),
        },
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trivia!(FunctionArgs, |this, leading, trailing| {
    match this {
        FunctionArgs::Parentheses {
            parentheses,
            arguments,
        } => FunctionArgs::Parentheses {
            parentheses: parentheses.update_trivia(leading, trailing),
            arguments: arguments.to_owned(),
        },
        FunctionArgs::String(token_reference) => {
            FunctionArgs::String(token_reference.update_trivia(leading, trailing))
        }
        FunctionArgs::TableConstructor(table_constructor) => {
            FunctionArgs::TableConstructor(table_constructor.update_trivia(leading, trailing))
        }
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trailing_trivia!(FunctionBody, |this, trailing| {
    this.to_owned()
        .with_end_token(this.end_token().update_trailing_trivia(trailing))
});

define_update_trivia!(FunctionCall, |this, leading, trailing| {
    let prefix = match leading {
        FormatTriviaType::NoChange => this.prefix().to_owned(),
        _ => this.prefix().update_leading_trivia(leading),
    };

    let mut suffixes: Vec<Suffix<'ast>> = this.suffixes().map(|x| x.to_owned()).collect();
    match trailing {
        FormatTriviaType::NoChange => (),
        _ => {
            if let Some(suffix) = suffixes.pop() {
                suffixes.push(suffix.update_trailing_trivia(trailing))
            }
        }
    };

    this.to_owned().with_prefix(prefix).with_suffixes(suffixes)
});

define_update_trivia!(FunctionName, |this, leading, trailing| {
    if let Some(method_name) = this.method_name() {
        let names = this.names().update_leading_trivia(leading);
        let method_name = method_name.update_trailing_trivia(trailing);
        this.to_owned()
            .with_names(names)
            .with_method(Some((this.method_colon().unwrap().to_owned(), method_name)))
    } else {
        let names = this
            .names()
            .update_leading_trivia(leading)
            .update_trailing_trivia(trailing);
        this.to_owned().with_names(names)
    }
});

define_update_trivia!(If, |this, leading, trailing| {
    this.to_owned()
        .with_if_token(this.if_token().update_leading_trivia(leading))
        .with_end_token(this.end_token().update_trailing_trivia(trailing))
});

define_update_trivia!(Index, |this, leading, trailing| {
    match this {
        Index::Brackets {
            brackets,
            expression,
        } => Index::Brackets {
            brackets: brackets.update_trivia(leading, trailing),
            expression: expression.to_owned(),
        },
        Index::Dot { dot, name } => Index::Dot {
            dot: dot.update_leading_trivia(leading),
            name: name.update_trailing_trivia(trailing),
        },
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trivia!(LastStmt, |this, leading, trailing| {
    match this {
        LastStmt::Break(token) => LastStmt::Break(token.update_trivia(leading, trailing)),
        LastStmt::Return(return_node) => {
            LastStmt::Return(return_node.update_trivia(leading, trailing))
        }
        #[cfg(feature = "luau")]
        LastStmt::Continue(token) => LastStmt::Continue(token.update_trivia(leading, trailing)),
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trivia!(MethodCall, |this, leading, trailing| {
    this.to_owned()
        .with_colon_token(this.colon_token().update_leading_trivia(leading))
        .with_args(this.args().update_trailing_trivia(trailing))
});

define_update_trivia!(Parameter, |this, leading, trailing| {
    match this {
        Parameter::Ellipse(token) => Parameter::Ellipse(token.update_trivia(leading, trailing)),
        Parameter::Name(token) => Parameter::Name(token.update_trivia(leading, trailing)),
        other => panic!("unknown node {:?}", other),
    }
});

define_update_leading_trivia!(Prefix, |this, leading| {
    match this {
        Prefix::Name(token_reference) => {
            Prefix::Name(token_reference.update_leading_trivia(leading))
        }
        Prefix::Expression(expression) => {
            Prefix::Expression(expression.update_leading_trivia(leading))
        }
        other => panic!("unknown node {:?}", other),
    }
});

impl<'ast, T> UpdateLeadingTrivia<'ast> for Punctuated<'ast, T>
where
    T: UpdateLeadingTrivia<'ast> + Clone,
{
    fn update_leading_trivia(&self, leading: FormatTriviaType<'ast>) -> Self {
        let mut punctuated = Punctuated::new();
        let mut pairs = self.to_owned().into_pairs();

        // Retrieve first item and add leading trivia
        if let Some(first_pair) = pairs.next() {
            let updated_pair = first_pair.map(|value| value.update_leading_trivia(leading));
            punctuated.push(updated_pair);
        };

        // Add back the rest of the values
        for pair in pairs {
            punctuated.push(full_moon::ast::punctuated::Pair::new(
                pair.value().clone(),
                pair.punctuation().map(|x| x.to_owned()),
            ))
        }

        punctuated
    }
}

impl<'ast, T> UpdateTrailingTrivia<'ast> for Punctuated<'ast, T>
where
    T: UpdateTrailingTrivia<'ast> + Clone,
{
    fn update_trailing_trivia(&self, trailing: FormatTriviaType<'ast>) -> Self {
        let mut punctuated = self.to_owned();

        // Add any trailing trivia to the end of the punctuated list
        if let Some(pair) = punctuated.pop() {
            let pair = pair.map(|value| value.update_trailing_trivia(trailing));
            punctuated.push(pair);
        }

        punctuated
    }
}

define_update_trivia!(Return, |this, leading, trailing| {
    if this.returns().is_empty() {
        this.to_owned()
            .with_token(this.token().update_trivia(leading, trailing))
    } else {
        this.to_owned()
            .with_token(this.token().update_leading_trivia(leading))
            .with_returns(this.returns().update_trailing_trivia(trailing))
    }
});

define_update_trivia!(Suffix, |this, leading, trailing| {
    match this {
        Suffix::Call(call) => Suffix::Call(call.update_trivia(leading, trailing)),
        Suffix::Index(index) => Suffix::Index(index.update_trivia(leading, trailing)),
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trivia!(TableConstructor, |this, leading, trailing| {
    this.to_owned()
        .with_braces(this.braces().update_trivia(leading, trailing))
});

define_update_leading_trivia!(UnOp, |this, leading| {
    match this {
        UnOp::Hash(token_reference) => UnOp::Hash(token_reference.update_leading_trivia(leading)),
        UnOp::Minus(token_reference) => UnOp::Minus(token_reference.update_leading_trivia(leading)),
        UnOp::Not(token_reference) => UnOp::Not(token_reference.update_leading_trivia(leading)),
        other => panic!("unknown node {:?}", other),
    }
});

define_update_leading_trivia!(Value, |this, leading| {
    match this {
        Value::Function((token, function_body)) => Value::Function((
            token.update_leading_trivia(leading),
            function_body.to_owned(),
        )),
        Value::FunctionCall(function_call) => {
            Value::FunctionCall(function_call.update_leading_trivia(leading))
        }
        Value::Number(token_reference) => {
            Value::Number(token_reference.update_leading_trivia(leading))
        }
        Value::ParenthesesExpression(expression) => {
            Value::ParenthesesExpression(expression.update_leading_trivia(leading))
        }
        Value::String(token_reference) => {
            Value::String(token_reference.update_leading_trivia(leading))
        }
        Value::Symbol(token_reference) => {
            Value::Symbol(token_reference.update_leading_trivia(leading))
        }
        Value::TableConstructor(table_constructor) => {
            Value::TableConstructor(table_constructor.update_leading_trivia(leading))
        }
        Value::Var(var) => Value::Var(var.update_leading_trivia(leading)),
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trailing_trivia!(Value, |this, trailing| {
    match this {
        Value::Function((token, function_body)) => Value::Function((
            token.to_owned(),
            function_body.update_trailing_trivia(trailing),
        )),
        Value::FunctionCall(function_call) => {
            Value::FunctionCall(function_call.update_trailing_trivia(trailing))
        }
        Value::Number(token_reference) => {
            Value::Number(token_reference.update_trailing_trivia(trailing))
        }
        Value::ParenthesesExpression(expression) => {
            Value::ParenthesesExpression(expression.update_trailing_trivia(trailing))
        }
        Value::String(token_reference) => {
            Value::String(token_reference.update_trailing_trivia(trailing))
        }
        Value::Symbol(token_reference) => {
            Value::Symbol(token_reference.update_trailing_trivia(trailing))
        }
        Value::TableConstructor(table_constructor) => {
            Value::TableConstructor(table_constructor.update_trailing_trivia(trailing))
        }
        Value::Var(var) => Value::Var(var.update_trailing_trivia(trailing)),
        other => panic!("unknown node {:?}", other),
    }
});

define_update_leading_trivia!(Var, |this, leading| {
    match this {
        Var::Name(token_reference) => Var::Name(token_reference.update_leading_trivia(leading)),
        Var::Expression(var_expresion) => {
            Var::Expression(var_expresion.update_leading_trivia(leading))
        }
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trailing_trivia!(Var, |this, trailing| {
    match this {
        Var::Name(token_reference) => Var::Name(token_reference.update_trailing_trivia(trailing)),
        Var::Expression(var_expression) => {
            Var::Expression(var_expression.update_trailing_trivia(trailing))
        }
        other => panic!("unknown node {:?}", other),
    }
});

define_update_trivia!(VarExpression, |this, leading, trailing| {
    let prefix = match leading {
        FormatTriviaType::NoChange => this.prefix().to_owned(),
        _ => this.prefix().update_leading_trivia(leading),
    };

    let mut suffixes: Vec<Suffix<'ast>> = this.suffixes().map(|x| x.to_owned()).collect();
    match trailing {
        FormatTriviaType::NoChange => (),
        _ => {
            if let Some(suffix) = suffixes.pop() {
                suffixes.push(suffix.update_trailing_trivia(trailing))
            }
        }
    };

    this.to_owned().with_prefix(prefix).with_suffixes(suffixes)
});

#[cfg(feature = "luau")]
define_update_trailing_trivia!(TypeInfo, |this, trailing| {
    match this {
        TypeInfo::Array { braces, type_info } => TypeInfo::Array {
            braces: braces.update_trailing_trivia(trailing),
            type_info: type_info.to_owned(),
        },
        TypeInfo::Basic(token_reference) => {
            TypeInfo::Basic(token_reference.update_trailing_trivia(trailing))
        }
        TypeInfo::Callback {
            parentheses,
            arguments,
            arrow,
            return_type,
        } => TypeInfo::Callback {
            parentheses: parentheses.to_owned(),
            arguments: arguments.to_owned(),
            arrow: arrow.to_owned(),
            return_type: Box::new(return_type.update_trailing_trivia(trailing)),
        },
        TypeInfo::Generic {
            base,
            arrows,
            generics,
        } => TypeInfo::Generic {
            base: base.to_owned(),
            arrows: arrows.update_trailing_trivia(trailing),
            generics: generics.to_owned(),
        },

        TypeInfo::Intersection {
            left,
            ampersand,
            right,
        } => TypeInfo::Intersection {
            left: left.to_owned(),
            ampersand: ampersand.to_owned(),
            right: Box::new(right.update_trailing_trivia(trailing)),
        },

        TypeInfo::Module {
            module,
            punctuation,
            type_info,
        } => TypeInfo::Module {
            module: module.to_owned(),
            punctuation: punctuation.to_owned(),
            type_info: Box::new(type_info.update_trailing_trivia(trailing)),
        },

        TypeInfo::Optional {
            base,
            question_mark,
        } => TypeInfo::Optional {
            base: base.to_owned(),
            question_mark: question_mark.update_trailing_trivia(trailing),
        },

        TypeInfo::Table { braces, fields } => TypeInfo::Table {
            braces: braces.update_trailing_trivia(trailing),
            fields: fields.to_owned(),
        },

        TypeInfo::Typeof {
            typeof_token,
            parentheses,
            inner,
        } => TypeInfo::Typeof {
            typeof_token: typeof_token.to_owned(),
            parentheses: parentheses.update_trailing_trivia(trailing),
            inner: inner.to_owned(),
        },

        TypeInfo::Tuple { parentheses, types } => TypeInfo::Tuple {
            parentheses: parentheses.update_trailing_trivia(trailing),
            types: types.to_owned(),
        },

        TypeInfo::Union { left, pipe, right } => TypeInfo::Union {
            left: left.to_owned(),
            pipe: pipe.to_owned(),
            right: Box::new(right.update_trailing_trivia(trailing)),
        },

        TypeInfo::Variadic { ellipse, type_info } => TypeInfo::Variadic {
            ellipse: ellipse.to_owned(),
            type_info: Box::new(type_info.update_trailing_trivia(trailing)),
        },

        other => panic!("unknown node {:?}", other),
    }
});

#[cfg(feature = "luau")]
define_update_trailing_trivia!(IndexedTypeInfo, |this, trailing| {
    match this {
        IndexedTypeInfo::Basic(token_reference) => {
            IndexedTypeInfo::Basic(token_reference.update_trailing_trivia(trailing))
        }
        IndexedTypeInfo::Generic {
            base,
            arrows,
            generics,
        } => IndexedTypeInfo::Generic {
            base: base.to_owned(),
            arrows: arrows.update_trailing_trivia(trailing),
            generics: generics.to_owned(),
        },

        other => panic!("unknown node {:?}", other),
    }
});

#[cfg(feature = "luau")]
define_update_trailing_trivia!(TypeAssertion, |this, trailing| {
    this.to_owned()
        .with_cast_to(this.cast_to().update_trailing_trivia(trailing))
});

#[cfg(feature = "luau")]
define_update_trailing_trivia!(TypeSpecifier, |this, trailing| {
    this.to_owned()
        .with_type_info(this.type_info().update_trailing_trivia(trailing))
});
