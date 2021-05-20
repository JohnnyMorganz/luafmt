use crate::{
    check_should_format,
    context::{create_indent_trivia, create_newline_trivia, Context},
    fmt_symbol,
    formatters::{
        assignment::hang_punctuated_list,
        expression::format_expression,
        general::{format_symbol, try_format_punctuated},
        stmt::format_stmt,
        trivia::{
            strip_trivia, FormatTriviaType, UpdateLeadingTrivia, UpdateTrailingTrivia, UpdateTrivia,
        },
        trivia_util,
    },
    shape::Shape,
};
use full_moon::ast::{
    punctuated::Punctuated, Block, Expression, LastStmt, Prefix, Return, Stmt, Var,
};
use full_moon::tokenizer::TokenType;
use full_moon::tokenizer::{Token, TokenReference};
#[cfg(feature = "luau")]
use std::borrow::Cow;

macro_rules! update_first_token {
    ($enum:ident, $var:ident, $token:expr, $update_method:ident) => {{
        let leading_trivia = trivia_remove_leading_newlines($token.leading_trivia().collect());
        let new_token = $token.update_leading_trivia(FormatTriviaType::Replace(leading_trivia));
        Stmt::$enum($var.$update_method(new_token))
    }};
}

pub fn format_return<'ast>(
    ctx: &Context,
    return_node: &Return<'ast>,
    shape: Shape,
) -> Return<'ast> {
    if return_node.returns().is_empty() {
        let token = fmt_symbol!(ctx, return_node.token(), "return", shape);
        Return::new().with_token(token)
    } else {
        let token = fmt_symbol!(ctx, return_node.token(), "return ", shape);

        let shape = shape + (strip_trivia(return_node.token()).to_string().len() + 1); // 1 = " "
        let mut formatted_returns =
            try_format_punctuated(ctx, return_node.returns(), shape, format_expression);

        // Determine if we need to hang the condition
        let require_multiline_expression = shape
            .take_first_line(&strip_trivia(&formatted_returns))
            .over_budget();

        if require_multiline_expression {
            formatted_returns = hang_punctuated_list(ctx, return_node.returns(), shape);
        }

        Return::new()
            .with_token(token)
            .with_returns(formatted_returns)
    }
}

pub fn format_last_stmt_<'ast>(
    ctx: &Context,
    last_stmt: &LastStmt<'ast>,
    shape: Shape,
    trivia: (FormatTriviaType<'ast>, FormatTriviaType<'ast>),
) -> LastStmt<'ast> {
    check_should_format!(ctx, last_stmt);

    // Calculate trivia
    let leading_trivia = trivia.0;
    let trailing_trivia = trivia.1;

    match last_stmt {
        LastStmt::Break(token) => LastStmt::Break(
            fmt_symbol!(ctx, token, "break", shape).update_trivia(leading_trivia, trailing_trivia),
        ),

        LastStmt::Return(return_node) => LastStmt::Return(
            format_return(ctx, return_node, shape).update_trivia(leading_trivia, trailing_trivia),
        ),

        #[cfg(feature = "luau")]
        LastStmt::Continue(token) => LastStmt::Continue(
            format_symbol(
                ctx,
                token,
                &TokenReference::new(
                    vec![],
                    Token::new(TokenType::Identifier {
                        identifier: Cow::Owned(String::from("continue")),
                    }),
                    vec![],
                ),
                shape,
            )
            .update_trivia(leading_trivia, trailing_trivia),
        ),

        other => panic!("unknown node {:?}", other),
    }
}

pub fn format_last_stmt<'ast>(
    ctx: &Context,
    last_stmt: &LastStmt<'ast>,
    shape: Shape,
) -> LastStmt<'ast> {
    // Calculate trivia
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let trailing_trivia = vec![create_newline_trivia(ctx)];
    let trivia = (
        FormatTriviaType::Append(leading_trivia),
        FormatTriviaType::Append(trailing_trivia),
    );

    format_last_stmt_(ctx, last_stmt, shape, trivia)
}

fn trivia_remove_leading_newlines<'ast>(trivia: Vec<&Token<'ast>>) -> Vec<Token<'ast>> {
    trivia
        .iter()
        .skip_while(|x| match x.token_type() {
            TokenType::Whitespace { ref characters } => characters.contains('\n'),
            _ => false,
        })
        .map(|x| x.to_owned().to_owned())
        .collect()
}

fn prefix_remove_leading_newlines<'ast>(prefix: &Prefix<'ast>) -> Prefix<'ast> {
    match prefix {
        Prefix::Name(token) => {
            let leading_trivia = trivia_remove_leading_newlines(token.leading_trivia().collect());
            Prefix::Name(token.update_leading_trivia(FormatTriviaType::Replace(leading_trivia)))
        }
        Prefix::Expression(expr) => Prefix::Expression(match expr {
            Expression::Parentheses {
                contained,
                expression,
            } => {
                let (start_parens, end_parens) = contained.tokens();
                let leading_trivia =
                    trivia_remove_leading_newlines(start_parens.leading_trivia().collect());
                Expression::Parentheses {
                    contained: full_moon::ast::span::ContainedSpan::new(
                        start_parens
                            .update_leading_trivia(FormatTriviaType::Replace(leading_trivia)),
                        end_parens.to_owned(),
                    ),
                    expression: Box::new(*expression.to_owned()),
                }
            }
            other => {
                unreachable!("got non-parentheses expression as prefix {:?}", other)
            }
        }),

        other => panic!("unknown node {:?}", other),
    }
}

fn var_remove_leading_newline(var: Var) -> Var {
    match var {
        Var::Name(token) => {
            let leading_trivia = trivia_remove_leading_newlines(token.leading_trivia().collect());
            Var::Name(token.update_leading_trivia(FormatTriviaType::Replace(leading_trivia)))
        }
        Var::Expression(var_expr) => {
            let prefix = prefix_remove_leading_newlines(var_expr.prefix());
            Var::Expression(var_expr.with_prefix(prefix))
        }
        other => panic!("unknown node {:?}", other),
    }
}

fn stmt_remove_leading_newlines(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Assignment(assignment) => {
            let mut var_list = Punctuated::new();

            for (idx, pair) in assignment.variables().pairs().enumerate() {
                if idx == 0 {
                    let pair = pair.to_owned().map(var_remove_leading_newline);
                    var_list.push(pair);
                } else {
                    var_list.push(pair.to_owned());
                }
            }

            Stmt::Assignment(assignment.with_variables(var_list))
        }
        Stmt::Do(do_block) => {
            update_first_token!(Do, do_block, do_block.do_token(), with_do_token)
        }
        Stmt::FunctionCall(function_call) => {
            let prefix = prefix_remove_leading_newlines(function_call.prefix());
            Stmt::FunctionCall(function_call.with_prefix(prefix))
        }
        Stmt::FunctionDeclaration(function_declaration) => {
            update_first_token!(
                FunctionDeclaration,
                function_declaration,
                function_declaration.function_token(),
                with_function_token
            )
        }
        Stmt::GenericFor(generic_for) => update_first_token!(
            GenericFor,
            generic_for,
            generic_for.for_token(),
            with_for_token
        ),
        Stmt::If(if_block) => {
            update_first_token!(If, if_block, if_block.if_token(), with_if_token)
        }
        Stmt::LocalAssignment(local_assignment) => update_first_token!(
            LocalAssignment,
            local_assignment,
            local_assignment.local_token(),
            with_local_token
        ),
        Stmt::LocalFunction(local_function) => update_first_token!(
            LocalFunction,
            local_function,
            local_function.local_token(),
            with_local_token
        ),
        Stmt::NumericFor(numeric_for) => update_first_token!(
            NumericFor,
            numeric_for,
            numeric_for.for_token(),
            with_for_token
        ),
        Stmt::Repeat(repeat_block) => {
            update_first_token!(
                Repeat,
                repeat_block,
                repeat_block.repeat_token(),
                with_repeat_token
            )
        }
        Stmt::While(while_block) => {
            update_first_token!(
                While,
                while_block,
                while_block.while_token(),
                with_while_token
            )
        }
        #[cfg(feature = "luau")]
        Stmt::CompoundAssignment(compound_assignment) => {
            let lhs = var_remove_leading_newline(compound_assignment.lhs().to_owned());
            Stmt::CompoundAssignment(compound_assignment.with_lhs(lhs))
        }

        #[cfg(feature = "luau")]
        Stmt::ExportedTypeDeclaration(exported_type_declaration) => update_first_token!(
            ExportedTypeDeclaration,
            exported_type_declaration,
            exported_type_declaration.export_token(),
            with_export_token
        ),
        #[cfg(feature = "luau")]
        Stmt::TypeDeclaration(type_declaration) => update_first_token!(
            TypeDeclaration,
            type_declaration,
            type_declaration.type_token(),
            with_type_token
        ),
        #[cfg(feature = "lua52")]
        Stmt::Goto(goto) => update_first_token!(Goto, goto, goto.goto_token(), with_goto_token),
        #[cfg(feature = "lua52")]
        Stmt::Label(label) => {
            update_first_token!(Label, label, label.left_colons(), with_left_colons)
        }
        other => panic!("unknown node {:?}", other),
    }
}

fn last_stmt_remove_leading_newlines(last_stmt: LastStmt) -> LastStmt {
    match last_stmt {
        LastStmt::Break(token) => {
            let leading_trivia = trivia_remove_leading_newlines(token.leading_trivia().collect());
            LastStmt::Break(token.update_leading_trivia(FormatTriviaType::Replace(leading_trivia)))
        }
        LastStmt::Return(return_node) => {
            let token = return_node
                .token()
                .update_leading_trivia(FormatTriviaType::Replace(trivia_remove_leading_newlines(
                    return_node.token().leading_trivia().collect(),
                )));

            LastStmt::Return(return_node.with_token(token))
        }
        #[cfg(feature = "luau")]
        LastStmt::Continue(token) => {
            let leading_trivia = trivia_remove_leading_newlines(token.leading_trivia().collect());
            LastStmt::Continue(
                token.update_leading_trivia(FormatTriviaType::Replace(leading_trivia)),
            )
        }
        other => panic!("unknown node {:?}", other),
    }
}

/// Formats a block node. Note: the given shape to the block formatter should already be at the correct indentation level
pub fn format_block<'ast>(ctx: &Context, block: &Block<'ast>, shape: Shape) -> Block<'ast> {
    let mut formatted_statements: Vec<(Stmt<'ast>, Option<TokenReference<'ast>>)> = Vec::new();
    let mut found_first_stmt = false;
    let mut stmt_iterator = block.stmts_with_semicolon().peekable();

    while let Some((stmt, semi)) = stmt_iterator.next() {
        let shape = shape.reset();
        let mut stmt = format_stmt(ctx, stmt, shape);

        // If this is the first stmt, then remove any leading newlines
        if !found_first_stmt {
            if ctx.should_format_node(&stmt) {
                stmt = stmt_remove_leading_newlines(stmt);
            }
            found_first_stmt = true;
        }

        // Need to check next statement if it is a function call, with a parameters expression as the prefix
        // If so, removing a semicolon may lead to ambiguous syntax
        // Ambiguous syntax can only occur if the current statement is a (Local)Assignment, FunctionCall or a Repeat block
        let require_semicolon = match stmt {
            Stmt::Assignment(_)
            | Stmt::LocalAssignment(_)
            | Stmt::FunctionCall(_)
            | Stmt::Repeat(_) => {
                let next_stmt = stmt_iterator.peek();
                match next_stmt {
                    Some((Stmt::FunctionCall(function_call), _)) => matches!(
                        function_call.prefix(),
                        Prefix::Expression(Expression::Parentheses { .. })
                    ),
                    _ => false,
                }
            }
            _ => false,
        };

        // If we have a semicolon, we need to push all the trailing trivia from the statement
        // and move it to the end of the semicolon
        let semicolon = match require_semicolon {
            true => {
                let (updated_stmt, trivia) = trivia_util::get_stmt_trailing_trivia(stmt);
                stmt = updated_stmt;
                Some(
                    match semi {
                        Some(semi) => crate::fmt_symbol!(ctx, semi, ";", shape),
                        None => TokenReference::symbol(";").expect("could not make semicolon"),
                    }
                    .update_trailing_trivia(FormatTriviaType::Append(trivia)),
                )
            }
            false => match semi {
                Some(semi) => {
                    // We used to have a semicolon, but now we are removing it
                    // We want to keep any old comments on the semicolon token, otherwise we will lose it
                    let (updated_stmt, trivia) = trivia_util::get_stmt_trailing_trivia(stmt);
                    stmt = updated_stmt;
                    // We will do a hack here, where we insert an empty token, and add all the remaining trivia onto it
                    Some(
                        format_symbol(
                            ctx,
                            semi,
                            &TokenReference::new(vec![], Token::new(TokenType::spaces(0)), vec![]),
                            shape,
                        )
                        .update_trailing_trivia(FormatTriviaType::Append(trivia)),
                    )
                }
                None => None,
            },
        };

        formatted_statements.push((stmt, semicolon))
    }

    // Drop the stmt_iterator as we do not need it anymore and we still need to use `block`
    drop(stmt_iterator);

    let formatted_last_stmt = match block.last_stmt_with_semicolon() {
        Some((last_stmt, semi)) => {
            let shape = shape.reset();
            let mut last_stmt = format_last_stmt(ctx, last_stmt, shape);
            // If this is the first stmt, then remove any leading newlines
            if !found_first_stmt && ctx.should_format_node(&last_stmt) {
                last_stmt = last_stmt_remove_leading_newlines(last_stmt);
            }
            // LastStmt will never need a semicolon
            // We need to check if we previously had a semicolon, and keep the comments if so
            let semicolon = match semi {
                Some(semi) => {
                    let (updated_last_stmt, trivia) =
                        trivia_util::get_last_stmt_trailing_trivia(last_stmt);
                    last_stmt = updated_last_stmt;

                    // We want to keep any old comments on the semicolon token, otherwise we will lose it
                    // We will do a hack here, where we replace the semicolon with an empty symbol
                    let semicolon_token = format_symbol(
                        ctx,
                        semi,
                        &TokenReference::new(vec![], Token::new(TokenType::spaces(0)), vec![]),
                        shape,
                    )
                    .update_trailing_trivia(FormatTriviaType::Append(trivia));
                    Some(semicolon_token)
                }
                None => None,
            };
            Some((last_stmt, semicolon))
        }
        None => None,
    };

    block
        .to_owned()
        .with_stmts(formatted_statements)
        .with_last_stmt(formatted_last_stmt)
}
