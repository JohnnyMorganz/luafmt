#[cfg(feature = "lua52")]
use crate::formatters::lua52::{format_goto, format_label};
#[cfg(feature = "luau")]
use crate::formatters::luau::{
    format_compound_assignment, format_exported_type_declaration, format_type_declaration_stmt,
    format_type_specifier,
};
use crate::{
    check_should_format,
    context::{create_indent_trivia, create_newline_trivia, Context},
    fmt_symbol,
    formatters::{
        assignment::{format_assignment, format_local_assignment},
        block::{format_block, format_last_stmt_},
        expression::{format_expression, hang_expression_trailing_newline},
        functions::{format_function_call, format_function_declaration, format_local_function},
        general::{
            format_end_token, format_punctuated_buffer, format_token_reference, EndTokenType,
        },
        trivia::{
            strip_trivia, FormatTriviaType, UpdateLeadingTrivia, UpdateTrailingTrivia, UpdateTrivia,
        },
        trivia_util,
    },
    shape::Shape,
};
use full_moon::ast::{
    Block, Do, ElseIf, Expression, FunctionCall, GenericFor, If, NumericFor, Repeat, Stmt, Value,
    While,
};
use full_moon::tokenizer::{Token, TokenReference, TokenType};

use super::trivia_util::contains_comments;

macro_rules! fmt_stmt {
    ($ctx:expr, $value:ident, $shape:ident, { $($(#[$inner:meta])* $operator:ident = $output:ident,)+ }) => {
        match $value {
            $(
                $(#[$inner])*
                Stmt::$operator(stmt) => Stmt::$operator($output($ctx, stmt, $shape)),
            )+
            other => panic!("unknown node {:?}", other),
        }
    };
}

/// Removes parentheses around a condition, if present.
/// Called only for condition expression (if ... then, while ... do, etc.)
fn remove_condition_parentheses(expression: Expression) -> Expression {
    match expression.to_owned() {
        Expression::Parentheses { expression, .. } => *expression,
        Expression::Value { value, .. } => match *value {
            Value::ParenthesesExpression(expression) => remove_condition_parentheses(expression),
            _ => expression,
        },
        _ => expression,
    }
}

/// Format a Do node
pub fn format_do_block<'ast>(ctx: &Context, do_block: &Do<'ast>, shape: Shape) -> Do<'ast> {
    // Create trivia
    let leading_trivia = FormatTriviaType::Append(vec![create_indent_trivia(ctx, shape)]);
    let trailing_trivia = FormatTriviaType::Append(vec![create_newline_trivia(ctx)]);

    let do_token = fmt_symbol!(ctx, do_block.do_token(), "do", shape)
        .update_trivia(leading_trivia.to_owned(), trailing_trivia.to_owned());
    let block_shape = shape.reset().increment_block_indent();
    let block = format_block(ctx, do_block.block(), block_shape);
    let end_token = format_end_token(ctx, do_block.end_token(), EndTokenType::BlockEnd, shape)
        .update_trivia(leading_trivia, trailing_trivia);

    do_block
        .to_owned()
        .with_do_token(do_token)
        .with_block(block)
        .with_end_token(end_token)
}

/// Format a GenericFor node
pub fn format_generic_for<'ast>(
    ctx: &Context,
    generic_for: &GenericFor<'ast>,
    shape: Shape,
) -> GenericFor<'ast> {
    // Create trivia
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let mut trailing_trivia = vec![create_newline_trivia(ctx)];

    // TODO: Should we actually update the shape here?
    let for_token = fmt_symbol!(ctx, generic_for.for_token(), "for ", shape)
        .update_leading_trivia(FormatTriviaType::Append(leading_trivia.to_owned()));
    let (formatted_names, mut names_comments_buf) =
        format_punctuated_buffer(ctx, generic_for.names(), shape, format_token_reference);

    #[cfg(feature = "luau")]
    let type_specifiers = generic_for
        .type_specifiers()
        .map(|x| match x {
            Some(type_specifier) => Some(format_type_specifier(ctx, type_specifier, shape)),
            None => None,
        })
        .collect();

    let in_token = fmt_symbol!(ctx, generic_for.in_token(), " in ", shape);
    let (formatted_expr_list, mut expr_comments_buf) =
        format_punctuated_buffer(ctx, generic_for.expressions(), shape, format_expression);

    // Create comments buffer and append to end of do token
    names_comments_buf.append(&mut expr_comments_buf);
    // Append trailing trivia to the end
    names_comments_buf.append(&mut trailing_trivia);

    let do_token = fmt_symbol!(ctx, generic_for.do_token(), " do", shape)
        .update_trailing_trivia(FormatTriviaType::Append(names_comments_buf));

    let block_shape = shape.reset().increment_block_indent();
    let block = format_block(ctx, generic_for.block(), block_shape);

    let end_token = format_end_token(ctx, generic_for.end_token(), EndTokenType::BlockEnd, shape)
        .update_trivia(
            FormatTriviaType::Append(leading_trivia),
            FormatTriviaType::Append(vec![create_newline_trivia(ctx)]), // trailing_trivia was emptied when it was appended to names_comment_buf
        );

    let generic_for = generic_for
        .to_owned()
        .with_for_token(for_token)
        .with_names(formatted_names)
        .with_in_token(in_token)
        .with_expressions(formatted_expr_list)
        .with_do_token(do_token)
        .with_block(block)
        .with_end_token(end_token);
    #[cfg(feature = "luau")]
    let generic_for = generic_for.with_type_specifiers(type_specifiers);
    generic_for
}

/// Formats an ElseIf node - This must always reside within format_if
fn format_else_if<'ast>(ctx: &Context, else_if_node: &ElseIf<'ast>, shape: Shape) -> ElseIf<'ast> {
    // Calculate trivia
    let shape = shape.reset();
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let trailing_trivia = vec![create_newline_trivia(ctx)];

    // Remove parentheses around the condition
    let condition = remove_condition_parentheses(else_if_node.condition().to_owned());

    // Determine if we need to hang the condition
    let singleline_shape = shape + (7 + 5 + strip_trivia(&condition).to_string().len()); // 7 = "elseif ", 5 = " then"
    let require_multiline_expression = singleline_shape.over_budget()
        || trivia_util::expression_contains_inline_comments(&condition);

    let (else_if_trailing_trivia, then_text) = if require_multiline_expression {
        (vec![create_newline_trivia(ctx)], "then")
    } else {
        (vec![Token::new(TokenType::spaces(1))], " then")
    };

    let formatted_else_if_token = format_end_token(
        ctx,
        else_if_node.else_if_token(),
        EndTokenType::BlockEnd,
        shape,
    )
    .update_leading_trivia(FormatTriviaType::Append(leading_trivia.to_owned()))
    .update_trailing_trivia(FormatTriviaType::Append(else_if_trailing_trivia));

    let formatted_condition = if require_multiline_expression {
        // Reset the shape onto a new line, and increment the additional indent level
        let shape = shape.reset().increment_additional_indent();
        hang_expression_trailing_newline(ctx, &condition, shape, None).update_leading_trivia(
            FormatTriviaType::Append(vec![create_indent_trivia(ctx, shape)]),
        )
    } else {
        format_expression(ctx, &condition, shape + 7) // 7 = "elseif "
    };

    let formatted_then_token = fmt_symbol!(ctx, else_if_node.then_token(), then_text, shape)
        .update_trivia(
            if require_multiline_expression {
                FormatTriviaType::Append(leading_trivia)
            } else {
                FormatTriviaType::NoChange
            },
            FormatTriviaType::Append(trailing_trivia),
        );

    let block_shape = shape.reset().increment_block_indent();
    let block = format_block(ctx, else_if_node.block(), block_shape);

    else_if_node
        .to_owned()
        .with_else_if_token(formatted_else_if_token)
        .with_condition(formatted_condition)
        .with_then_token(formatted_then_token)
        .with_block(block)
}

/// Checks to see whether an [`If`] statement matches the structure of an "if guard".
/// ```lua
/// if condition then return expr end
/// ```
/// If we have an if guard, we can keep the statement single line - provided that a few key observations hold:
/// - There is no elseif/else block
/// - The body of the if block only contains a single [`LastStmt`] (note, this LastStmt may have multiple expressions e.g. `return foo, bar`)
/// - There are no internal comments within the block
/// We will also check if the statement surpasses the column width, or if the condition required multilining (handled outside of this function)
fn is_if_guard(if_node: &If) -> bool {
    let valid_last_stmt = if let Some(last_stmt) = if_node.block().last_stmt() {
        !contains_comments(last_stmt)
    } else {
        false
    };

    if_node.else_if().is_none()
        && if_node.else_block().is_none()
        && if_node.block().stmts().next().is_none()
        && valid_last_stmt
        && !contains_comments(if_node.then_token())
}

/// Formats an if statement onto a single line.
/// This function performs no checking to see if we *should* do this - it will just return a singleline if statement.
fn format_singeline_if<'ast>(ctx: &Context, if_node: &If<'ast>, shape: Shape) -> If<'ast> {
    // Calculate trivia
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let trailing_trivia = vec![create_newline_trivia(ctx)];

    let if_token = fmt_symbol!(ctx, if_node.if_token(), "if ", shape)
        .update_leading_trivia(FormatTriviaType::Append(leading_trivia.to_owned()));

    // Remove parentheses around the condition
    let condition = remove_condition_parentheses(if_node.condition().to_owned());
    let condition = format_expression(ctx, &condition, shape + 3); // 3 = "if "

    let then_token = fmt_symbol!(ctx, if_node.then_token(), " then", shape);

    // Don't reset the block shape, as we are keeping the block single line
    let block_shape = shape
        + (strip_trivia(&if_token).to_string().len()
            + 1 // 1 = Space after if
            + condition.to_string().len()
            + then_token.to_string().len()
            + 1); // 1 = Space after then

    // Rather than deferring to `format_block()`, since we know that there is only a single LastStmt in the block, we can format it immediately
    // We need to modify the formatted LastStmt, since it will have automatically added leading/trailing trivia we don't want
    // We assume that there is only a laststmt present in the block - the callee of this function should have already checked for this
    let last_stmt = if_node
        .block()
        .last_stmt()
        .expect("somehow have no last stmt in singleline if");
    let last_stmt = format_last_stmt_(
        ctx,
        last_stmt,
        block_shape,
        (
            FormatTriviaType::Append(vec![Token::new(TokenType::spaces(1))]),
            FormatTriviaType::Append(vec![Token::new(TokenType::spaces(1))]),
        ),
    );
    let block = Block::new().with_last_stmt(Some((last_stmt, None)));

    let end_token = format_end_token(ctx, if_node.end_token(), EndTokenType::BlockEnd, shape)
        .update_trailing_trivia(FormatTriviaType::Append(trailing_trivia.to_owned()));

    if_node
        .to_owned()
        .with_if_token(if_token)
        .with_condition(condition)
        .with_then_token(then_token)
        .with_block(block)
        .with_end_token(end_token)
}

/// Format an If node
pub fn format_if<'ast>(ctx: &Context, if_node: &If<'ast>, shape: Shape) -> If<'ast> {
    // Calculate trivia
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let trailing_trivia = vec![create_newline_trivia(ctx)];

    // Remove parentheses around the condition
    let condition = remove_condition_parentheses(if_node.condition().to_owned());

    // Determine if we need to hang the condition
    let singleline_shape = shape + (3 + 5 + strip_trivia(&condition).to_string().len()); // 3 = "if ", 5 = " then"
    let require_multiline_expression = singleline_shape.over_budget()
        || trivia_util::expression_contains_inline_comments(&condition);

    // Handle check to see if we should format the if statement on a single line
    if !require_multiline_expression && is_if_guard(if_node) {
        let singleline_if = format_singeline_if(ctx, if_node, shape);

        // See if it fits under the column width. If it does, bail early and return this singleline if
        if !shape
            .add_width(strip_trivia(&singleline_if).to_string().len())
            .over_budget()
        {
            return singleline_if;
        }
    }

    let (if_text, then_text) = if require_multiline_expression {
        ("if\n", "then")
    } else {
        ("if ", " then")
    };

    let if_token = fmt_symbol!(ctx, if_node.if_token(), if_text, shape)
        .update_leading_trivia(FormatTriviaType::Append(leading_trivia.to_owned()));

    let condition = if require_multiline_expression {
        // Reset the shape onto a new line, and increment the additional indent level
        let shape = shape.reset().increment_additional_indent();
        hang_expression_trailing_newline(ctx, &condition, shape, None).update_leading_trivia(
            FormatTriviaType::Append(vec![create_indent_trivia(ctx, shape)]),
        )
    } else {
        format_expression(ctx, &condition, shape + 3) // 3 = "if "
    };

    let then_token = fmt_symbol!(ctx, if_node.then_token(), then_text, shape).update_trivia(
        if require_multiline_expression {
            FormatTriviaType::Append(leading_trivia.to_owned())
        } else {
            FormatTriviaType::NoChange
        },
        FormatTriviaType::Append(trailing_trivia.to_owned()),
    );

    let block_shape = shape.reset().increment_block_indent();
    let block = format_block(ctx, if_node.block(), block_shape);

    let end_token = format_end_token(ctx, if_node.end_token(), EndTokenType::BlockEnd, shape)
        .update_trivia(
            FormatTriviaType::Append(leading_trivia.to_owned()),
            FormatTriviaType::Append(trailing_trivia.to_owned()),
        );

    let else_if = if_node.else_if().map(|else_if| {
        else_if
            .iter()
            .map(|else_if| format_else_if(ctx, else_if, shape))
            .collect()
    });

    let (else_token, else_block) = match (if_node.else_token(), if_node.else_block()) {
        (Some(else_token), Some(else_block)) => {
            let else_token = format_end_token(ctx, else_token, EndTokenType::BlockEnd, shape)
                .update_trivia(
                    FormatTriviaType::Append(leading_trivia),
                    FormatTriviaType::Append(trailing_trivia),
                );
            let else_block_shape = shape.reset().increment_block_indent();
            let else_block = format_block(ctx, else_block, else_block_shape);

            (Some(else_token), Some(else_block))
        }
        (None, None) => (None, None),
        _ => unreachable!("Got an else token with no else block or vice versa"),
    };

    if_node
        .to_owned()
        .with_if_token(if_token)
        .with_condition(condition)
        .with_then_token(then_token)
        .with_block(block)
        .with_else_if(else_if)
        .with_else_token(else_token)
        .with_else(else_block)
        .with_end_token(end_token)
}

/// Format a NumericFor node
pub fn format_numeric_for<'ast>(
    ctx: &Context,
    numeric_for: &NumericFor<'ast>,
    shape: Shape,
) -> NumericFor<'ast> {
    // Create trivia
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let trailing_trivia = vec![create_newline_trivia(ctx)];

    let for_token = fmt_symbol!(ctx, numeric_for.for_token(), "for ", shape)
        .update_leading_trivia(FormatTriviaType::Append(leading_trivia.to_owned()));
    let index_variable = format_token_reference(ctx, numeric_for.index_variable(), shape);

    #[cfg(feature = "luau")]
    let type_specifier = match numeric_for.type_specifier() {
        Some(type_specifier) => Some(format_type_specifier(ctx, type_specifier, shape)),
        None => None,
    };

    // TODO: Should we actually update the shape here?
    let equal_token = fmt_symbol!(ctx, numeric_for.equal_token(), " = ", shape);
    let start = format_expression(ctx, numeric_for.start(), shape);
    let start_end_comma = fmt_symbol!(ctx, numeric_for.start_end_comma(), ", ", shape);
    let end = format_expression(ctx, numeric_for.end(), shape);

    let (end_step_comma, step) = match (numeric_for.end_step_comma(), numeric_for.step()) {
        (Some(end_step_comma), Some(step)) => (
            Some(fmt_symbol!(ctx, end_step_comma, ", ", shape)),
            Some(format_expression(ctx, step, shape)),
        ),
        (None, None) => (None, None),
        _ => unreachable!("Got numeric for end step comma with no step or vice versa"),
    };

    let do_token = fmt_symbol!(ctx, numeric_for.do_token(), " do", shape)
        .update_trailing_trivia(FormatTriviaType::Append(trailing_trivia.to_owned()));
    let block_shape = shape.reset().increment_block_indent();
    let block = format_block(ctx, numeric_for.block(), block_shape);
    let end_token = format_end_token(ctx, numeric_for.end_token(), EndTokenType::BlockEnd, shape)
        .update_trivia(
            FormatTriviaType::Append(leading_trivia),
            FormatTriviaType::Append(trailing_trivia),
        );

    let numeric_for = numeric_for
        .to_owned()
        .with_for_token(for_token)
        .with_index_variable(index_variable)
        .with_equal_token(equal_token)
        .with_start(start)
        .with_start_end_comma(start_end_comma)
        .with_end(end)
        .with_end_step_comma(end_step_comma)
        .with_step(step)
        .with_do_token(do_token)
        .with_block(block)
        .with_end_token(end_token);
    #[cfg(feature = "luau")]
    let numeric_for = numeric_for.with_type_specifier(type_specifier);

    numeric_for
}

/// Format a Repeat node
pub fn format_repeat_block<'ast>(
    ctx: &Context,
    repeat_block: &Repeat<'ast>,
    shape: Shape,
) -> Repeat<'ast> {
    // Calculate trivia
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let trailing_trivia = vec![create_newline_trivia(ctx)];

    let repeat_token = fmt_symbol!(ctx, repeat_block.repeat_token(), "repeat", shape)
        .update_trivia(
            FormatTriviaType::Append(leading_trivia.to_owned()),
            FormatTriviaType::Append(trailing_trivia.to_owned()),
        );
    let block_shape = shape.reset().increment_block_indent();
    let block = format_block(ctx, repeat_block.block(), block_shape);
    let until_token = fmt_symbol!(ctx, repeat_block.until_token(), "until ", shape)
        .update_leading_trivia(FormatTriviaType::Append(leading_trivia.to_owned()));

    // Remove parentheses around the condition
    let condition = remove_condition_parentheses(repeat_block.until().to_owned());

    // Determine if we need to hang the condition
    let singleline_shape = shape + (6 + strip_trivia(&condition).to_string().len()); // 6 = "until "
    let require_multiline_expression = singleline_shape.over_budget()
        || trivia_util::expression_contains_inline_comments(&condition);

    let shape = shape + 6; // 6 = "until "
    let until = match require_multiline_expression {
        true => {
            let shape = shape.increment_additional_indent();
            hang_expression_trailing_newline(ctx, &condition, shape, None)
        }
        false => format_expression(ctx, &condition, shape)
            .update_trailing_trivia(FormatTriviaType::Append(trailing_trivia)),
    };

    repeat_block
        .to_owned()
        .with_repeat_token(repeat_token)
        .with_block(block)
        .with_until_token(until_token)
        .with_until(until)
}

/// Format a While node
pub fn format_while_block<'ast>(
    ctx: &Context,
    while_block: &While<'ast>,
    shape: Shape,
) -> While<'ast> {
    // Calculate trivia
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let trailing_trivia = vec![create_newline_trivia(ctx)];

    // Remove parentheses around the condition
    let condition = remove_condition_parentheses(while_block.condition().to_owned());

    let singleline_while_token = fmt_symbol!(ctx, while_block.while_token(), "while ", shape);
    let singleline_condition = format_expression(ctx, &condition, shape + 6);
    let singleline_do_token = fmt_symbol!(ctx, while_block.do_token(), " do", shape);

    // Determine if we need to hang the condition
    let singleline_shape = shape + (6 + 3 + strip_trivia(&singleline_condition).to_string().len()); // 6 = "while ", 3 = " do"
    let require_multiline_expression = singleline_shape.over_budget()
        || trivia_util::expression_contains_inline_comments(&condition);

    let while_token = match require_multiline_expression {
        true => fmt_symbol!(ctx, while_block.while_token(), "while\n", shape),
        false => singleline_while_token,
    }
    .update_leading_trivia(FormatTriviaType::Append(leading_trivia.to_owned()));

    let condition = match require_multiline_expression {
        true => {
            let shape = shape.reset().increment_additional_indent();
            hang_expression_trailing_newline(ctx, &condition, shape, None).update_leading_trivia(
                FormatTriviaType::Append(vec![create_indent_trivia(ctx, shape)]),
            )
        }
        false => singleline_condition,
    };

    let do_token = match require_multiline_expression {
        true => fmt_symbol!(ctx, while_block.do_token(), "do", shape)
            .update_leading_trivia(FormatTriviaType::Append(leading_trivia.to_owned())),
        false => singleline_do_token,
    }
    .update_trailing_trivia(FormatTriviaType::Append(trailing_trivia.to_owned()));

    let block_shape = shape.reset().increment_block_indent();
    let block = format_block(ctx, while_block.block(), block_shape);

    let end_token = format_end_token(ctx, while_block.end_token(), EndTokenType::BlockEnd, shape)
        .update_trivia(
            FormatTriviaType::Append(leading_trivia),
            FormatTriviaType::Append(trailing_trivia),
        );

    while_block
        .to_owned()
        .with_while_token(while_token)
        .with_condition(condition)
        .with_do_token(do_token)
        .with_block(block)
        .with_end_token(end_token)
}

/// Wrapper around `format_function_call`, but also handles adding the trivia around the function call.
/// This can't be done in the original function, as function calls are not always statements, but can also be
/// in expressions.
pub fn format_function_call_stmt<'ast>(
    ctx: &Context,
    function_call: &FunctionCall<'ast>,
    shape: Shape,
) -> FunctionCall<'ast> {
    // Calculate trivia
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];
    let trailing_trivia = vec![create_newline_trivia(ctx)];

    format_function_call(ctx, function_call, shape).update_trivia(
        FormatTriviaType::Append(leading_trivia),
        FormatTriviaType::Append(trailing_trivia),
    )
}

pub fn format_stmt<'ast>(ctx: &Context, stmt: &Stmt<'ast>, shape: Shape) -> Stmt<'ast> {
    check_should_format!(ctx, stmt);

    fmt_stmt!(ctx, stmt, shape, {
        Assignment = format_assignment,
        Do = format_do_block,
        FunctionCall = format_function_call_stmt,
        FunctionDeclaration = format_function_declaration,
        GenericFor = format_generic_for,
        If = format_if,
        LocalAssignment = format_local_assignment,
        LocalFunction = format_local_function,
        NumericFor = format_numeric_for,
        Repeat = format_repeat_block,
        While = format_while_block,
        #[cfg(feature = "luau")] CompoundAssignment = format_compound_assignment,
        #[cfg(feature = "luau")] ExportedTypeDeclaration = format_exported_type_declaration,
        #[cfg(feature = "luau")] TypeDeclaration = format_type_declaration_stmt,
        #[cfg(feature = "lua52")] Goto = format_goto,
        #[cfg(feature = "lua52")] Label = format_label,
    })
}
