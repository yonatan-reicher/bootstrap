mod token_reader;

use crate::ast;
use crate::error::Report;
use crate::name::Name;
use crate::range::Range;
use crate::token::{Keyword, LToken, Symbol, Token};
use token_reader::TokenReader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadNameAfterImport(Range),
    NoIdentifierAfterDoubleColon(Range),
    ExpectedExpression(Range),
    NoTypeExpressionAfterColonInVariableDeclaration(Range),
    NoEqualsInVariableDeclaration(Range),
    CouldNotAssignTo(ast::Expr),
    NoNameAfterFor(Range),
    IfConditionDidNotEnd(Range),
    NoDoAfterIf(Range),
}

impl From<Error> for Report {
    fn from(error: Error) -> Report {
        match error {
            Error::BadNameAfterImport(range) => Report {
                message: "This import statement is missing what to import".to_string(),
                range,
                hint: Some(
                    "Import statements are of the form `import <module>::<sub-module>::<name>`"
                        .to_string(),
                ),
            },
            Error::NoIdentifierAfterDoubleColon(range) => Report {
                message: "This module access is missing what to access".to_string(),
                range,
                hint: Some(
                    "Module access is of the form `<module>::<sub-module>::<name>`".to_string(),
                ),
            },
            // TODO: Make this error more specific.
            Error::ExpectedExpression(range) => Report {
                message: "This is not the start of an expression".to_string(),
                range,
                hint: None,
            },
            Error::NoTypeExpressionAfterColonInVariableDeclaration(range) => Report {
                message: "This variable declaration is missing a type after the colon".to_string(),
                range,
                hint: Some(
                    "Variable declarations are of the form `<name> : <type> = <expression>`"
                        .to_string(),
                ),
            },
            Error::NoEqualsInVariableDeclaration(range) => Report {
                message: "This variable declaration is missing an equals sign".to_string(),
                range,
                hint: Some(
                    "Variable declarations are of the form `<name> : <type> = <expression>`"
                        .to_string(),
                ),
            },
            Error::CouldNotAssignTo(expr) => Report {
                message: format!("The expression '{expr:?}' cannot be assigned to"),
                range: expr.range(),
                hint: Some("Currently, only variables can be assigned to".to_string()),
            },
            Error::NoNameAfterFor(range) => Report {
                message: "This for loop is missing a name after the 'for' keyword".to_string(),
                range,
                hint: None,
            },
            Error::IfConditionDidNotEnd(range) => Report {
                message: "This if condition did not end".to_string(),
                range,
                hint: None,
            },
            Error::NoDoAfterIf(range) => Report {
                message: "This if statement is missing a 'do' keyword".to_string(),
                range,
                hint: Some(
                    "If statements are of the form 'if <condition> do { <body> }'".to_string(),
                ),
            },
        }
    }
}

pub fn parse(tokens: Vec<LToken>) -> Result<ast::Program, Vec<Error>> {
    let mut state = State {
        tokens: TokenReader::new(tokens),
        errors: Vec::new(),
    };

    let Ok(program) = parse_program(&mut state) else {
        return Err(state.errors);
    };

    if !state.errors.is_empty() {
        return Err(state.errors);
    }

    Ok(program)
}

fn parse_program(state: &mut State) -> Result<ast::Program, ()> {
    let mut statements = vec![];
    loop {
        skip_newlines(state);

        if state.current().is_none() {
            break Ok(ast::Program { statements });
        }

        let Ok(statement) = parse_statement(state) else {
            skip_until_top_level_newline(state);
            continue;
        };
        statements.push(statement);
    }
}

fn parse_statement(state: &mut State) -> Result<ast::Statement, ()> {
    if state.pop_token_eq(Keyword::Import) {
        return parse_import(state);
    }

    if state.pop_token_eq(Keyword::For) {
        return parse_for(state);
    }

    if let Some(statement) = parse_if(state)? {
        return Ok(statement);
    }

    let expr = parse_expression(state);

    if state.pop_token_eq(Symbol::Colon) {
        let type_ = parse_type_expr(state).and_then(|type_| {
            type_.map(Ok).unwrap_or_else(|| {
                let range = state.curr_range();
                state
                    .errors
                    .push(Error::NoTypeExpressionAfterColonInVariableDeclaration(
                        range,
                    ));
                skip_until(state, Symbol::Equal);
                Err(())
            })
        });

        if !state.pop_token_eq(Symbol::Equal) {
            let range = state.curr_range();
            state
                .errors
                .push(Error::NoEqualsInVariableDeclaration(range));
            return Err(());
        }

        let rhs = parse_expression(state);

        let Ok(ast::Expr::Var(name, _)) = &expr else {
            state.errors.push(Error::CouldNotAssignTo(expr?));
            return Err(());
        };

        return Ok(ast::Statement::VarDecl(name.first().unwrap().clone(), type_?, rhs?));
    }

    expr.map(ast::Statement::Expr)
}

fn parse_expression(state: &mut State) -> Result<ast::Expr, ()> {
    let Some(first) = parse_atom(state)? else {
        state.errors.push(Error::ExpectedExpression(state.curr_range()));
        return Err(());
    };

    let mut args = vec![];
    loop {
        let arg = parse_atom(state);
        args.push(match arg {
            Ok(Some(arg)) => Ok(arg),
            Ok(None) => break,
            Err(()) => Err(()),
        });
    }

    if args.is_empty() {
        return Ok(first);
    }

    Ok(ast::Expr::Apply {
        func: first.into(),
        args: args.into_iter().collect::<Result<_, _>>()?,
    })
}

fn parse_atom(state: &mut State) -> Result<Option<ast::Expr>, ()> {
    let start = state.curr_range();

    if let Some(literal) = parse_literal(state)? {
        let range = start | state.prev_range();
        return Ok(Some(ast::Expr::Literal(literal, range)));
    }

    if state.pop_token_eq(Symbol::OpenParen) {
        // TODO: Skip until close paren
        let expr = parse_expression(state)?;
        if !state.pop_token_eq(Symbol::CloseParen) {
            todo!()
        }
        return Ok(Some(expr));
    }

    if let Some(name) = parse_qualified_name(state)? {
        let range = start | state.prev_range();

        return Ok(Some(ast::Expr::Var(name, range)));
    }

    Ok(None)
}

fn parse_type_expr(state: &mut State) -> Result<Option<ast::TypeExpr>, ()> {
    if let Some(ident) = state.pop_token_ident() {
        return Ok(Some(ast::TypeExpr::Var(
            Name::from_str(ident),
            state.prev_range(),
        )));
    }

    Ok(None)
}

fn parse_import(state: &mut State) -> Result<ast::Statement, ()> {
    let name_start = state.curr_range();
    let Some(name) = parse_qualified_name(state)? else {
        state.errors.push(Error::BadNameAfterImport(name_start | state.curr_range()));
        return Err(());
    };

    Ok(ast::Statement::Import(name))
}

fn parse_for(state: &mut State) -> Result<ast::Statement, ()> {
    let name_start_range = state.curr_range();
    let name = state.pop_token_ident();

    skip_until(state, Keyword::In);
    let name_end_range = state.prev_range();
    if !state.pop_token_eq(Keyword::In) {
        todo!()
    }

    let start_expr = parse_expression(state)?;

    if !state.pop_token_eq(Symbol::DotDot) {
        todo!()
    }

    let end_expr = parse_expression(state)?;

    if !state.pop_token_eq(Keyword::Do) {
        todo!()
    }

    // Allow starting the brace on a new line.
    let indent = state.indent();
    state.pop_indent_same(indent);

    // TODO: Allow this to be a block.
    let body = parse_block(state);

    let Some(name) = name else {
        state.errors.push(Error::NoNameAfterFor(name_start_range | name_end_range));
        return Err(());
    };

    let Some(body) = body? else {
        todo!();
    };

    Ok(ast::Statement::For(
        Name::from_str(name),
        start_expr,
        end_expr,
        body,
    ))
}

fn parse_if(state: &mut State) -> Result<Option<ast::Statement>, ()> {
    if !state.pop_token_eq(Keyword::If) {
        return Ok(None);
    }

    let condition = parse_expression(state);
    let range_after_expr = state.curr_range();

    let skipped = skip_until_pred(state, |token| {
        token == &Symbol::OpenCurly.into() || token == &Keyword::Do.into()
    });

    if !state.pop_token_eq(Keyword::Do) {
        state
            .errors
            .push(Error::NoDoAfterIf(range_after_expr | state.curr_range()));
        return Err(());
    }

    if skipped {
        state.errors.push(Error::IfConditionDidNotEnd(
            range_after_expr | state.curr_range(),
        ));
    }

    let block = parse_block(state);

    // TODO: Parse else

    let Some(block) = block? else {
        todo!()
    };

    Ok(Some(ast::Statement::If(condition?, block, None)))
}

fn parse_block(state: &mut State) -> Result<Option<Vec<ast::Statement>>, ()> {
    if !state.pop_token_eq(Symbol::OpenCurly) {
        return Ok(None);
    }

    let outer_indent = state.indent();
    if !state.pop_indent_in() {
        todo!()
    }
    let inner_indent = state.indent();

    let mut statements = vec![];
    while !state.pop_indent_same(outer_indent) {
        let stmt = parse_statement(state);
        state.pop_indent_same(inner_indent);
        statements.push(stmt);
    }

    if !state.pop_token_eq(Symbol::CloseCurly) {
        todo!()
    }

    let statements = statements.into_iter().collect::<Result<_, _>>()?;
    Ok(Some(statements))
}

fn parse_qualified_name(state: &mut State) -> Result<Option<ast::QualifiedName>, ()> {
    let Some(first) = state.pop_token_ident() else {
        return Ok(None);
    };

    let mut ret = vec![Name::from_str(first)];
    while state.pop_token_eq(Symbol::DoubleColon) {
        let Some(next) = state.pop_token_ident() else {
            state.errors.push(Error::NoIdentifierAfterDoubleColon(state.curr_range()));
            return Err(());
        };
        ret.push(Name::from_str(next));
    }

    Ok(Some(ret))
}

fn parse_literal(state: &mut State) -> Result<Option<ast::Literal>, ()> {
    if let Some(string) = state.pop_token_string() {
        return Ok(Some(ast::Literal::Str(Name::from_str(string))));
    }

    if let Some(int) = state.pop_token_int() {
        return Ok(Some(ast::Literal::I32(int)));
    };

    Ok(None)
}

fn skip_newlines(state: &mut State) {
    while state.pop_token_newline().is_some() {}
}

fn skip_until_top_level_newline(state: &mut State) {
    use crate::token::NewLine as L;
    while let Some(curr) = state.pop_token() {
        if curr == Token::NewLine(L::NewLine { indent: 0 }) {
            break;
        }

        // Skip braces, parens, and brackets
        if curr == Symbol::OpenCurly.into() {
            skip_until(state, Symbol::CloseCurly);
            continue;
        } else if curr == Symbol::OpenParen.into() {
            skip_until(state, Symbol::CloseParen);
            continue;
        } /* else if curr == Some(Symbol::OpenBracket.into()) {
              skip_until(state, Symbol::CloseBracket);
              continue;
          } */
        // TODO: Return this.
    }
}

fn skip_until<'a>(state: &mut State, token: impl Into<Token<'a>>) {
    let token = token.into();
    while state.curr_token() != Some(token) {
        if state.pop().is_none() {
            break;
        }
    }
}

fn skip_until_pred(state: &mut State, predicate: impl Fn(&Token) -> bool) -> bool {
    let mut ret = false;
    while state.curr_token().filter(|t| !predicate(t)).is_some() {
        ret = true;
        if state.pop().is_none() {
            break;
        }
    }
    ret
}

struct State<'source> {
    tokens: TokenReader<'source>,
    errors: Vec<Error>,
}

impl<'a> std::ops::Deref for State<'a> {
    type Target = TokenReader<'a>;

    fn deref(&self) -> &Self::Target {
        &self.tokens
    }
}

impl<'a> std::ops::DerefMut for State<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tokens
    }
}