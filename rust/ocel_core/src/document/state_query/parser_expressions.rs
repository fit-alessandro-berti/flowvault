
impl QueryParser {
    fn parse_state_query(&mut self) -> OcelResult<StateQuery> {
        self.expect_keyword("STATE")?;
        let attribute_name = self.parse_identifier()?;
        self.expect_keyword("FOR")?;
        self.expect_keyword("LEADING")?;
        self.expect_keyword("OBJECT")?;
        self.expect_keyword("TYPE")?;
        let leading_object_type = self.parse_type_name()?;
        self.expect_keyword("AS")?;
        self.expect_keyword("CASE")?;

        let mut branches = Vec::new();
        while self.consume_keyword("WHEN") {
            let condition = self.parse_expr()?;
            self.expect_keyword("THEN")?;
            let value = self.parse_value_expr()?;
            branches.push(StateBranch { condition, value });
        }

        if branches.is_empty() {
            return Err(OcelError::new(
                "state query must contain at least one WHEN branch",
            ));
        }

        let else_value = if self.consume_keyword("ELSE") {
            Some(self.parse_value_expr()?)
        } else {
            None
        };

        self.expect_keyword("END")?;
        self.consume_token(&Token::Semicolon);
        if !self.is_done() {
            return Err(OcelError::new("unexpected tokens after END in state query"));
        }

        Ok(StateQuery {
            attribute_name,
            leading_object_type,
            branches,
            else_value,
        })
    }

    fn parse_expr(&mut self) -> OcelResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> OcelResult<Expr> {
        let mut expr = self.parse_and()?;
        while self.consume_keyword("OR") {
            let right = self.parse_and()?;
            expr = Expr::Or(Box::new(expr), Box::new(right));
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> OcelResult<Expr> {
        let mut expr = self.parse_not()?;
        while self.consume_keyword("AND") {
            let right = self.parse_not()?;
            expr = Expr::And(Box::new(expr), Box::new(right));
        }
        Ok(expr)
    }

    fn parse_not(&mut self) -> OcelResult<Expr> {
        if self.consume_keyword("NOT") {
            Ok(Expr::Not(Box::new(self.parse_not()?)))
        } else {
            self.parse_predicate()
        }
    }

    fn parse_predicate(&mut self) -> OcelResult<Expr> {
        if self.consume_token(&Token::LParen) {
            let expr = self.parse_expr()?;
            self.expect_token(&Token::RParen)?;
            return Ok(expr);
        }

        let left = self.parse_value_expr()?;

        if self.consume_keyword("IS") {
            let negated = self.consume_keyword("NOT");
            self.expect_keyword("NULL")?;
            return Ok(Expr::IsNull {
                value: left,
                negated,
            });
        }

        let op = self.parse_compare_op()?;
        let right = self.parse_value_expr()?;
        Ok(Expr::Compare { left, op, right })
    }

    fn parse_compare_op(&mut self) -> OcelResult<CompareOp> {
        if self.consume_token(&Token::Eq) {
            Ok(CompareOp::Eq)
        } else if self.consume_token(&Token::Ne) {
            Ok(CompareOp::Ne)
        } else if self.consume_token(&Token::Le) {
            Ok(CompareOp::Le)
        } else if self.consume_token(&Token::Lt) {
            Ok(CompareOp::Lt)
        } else if self.consume_token(&Token::Ge) {
            Ok(CompareOp::Ge)
        } else if self.consume_token(&Token::Gt) {
            Ok(CompareOp::Gt)
        } else if self.consume_keyword("LIKE") {
            Ok(CompareOp::Like)
        } else {
            Err(OcelError::new(
                "expected comparison operator in state query predicate",
            ))
        }
    }

    fn parse_value_expr(&mut self) -> OcelResult<ValueExpr> {
        match self.next().cloned() {
            Some(Token::String(value)) => Ok(ValueExpr::Literal(QueryValue::String(value))),
            Some(Token::Number(value)) => Ok(ValueExpr::Literal(QueryValue::Number(value))),
            Some(Token::Identifier(identifier)) => {
                if identifier.eq_ignore_ascii_case("TRUE") {
                    return Ok(ValueExpr::Literal(QueryValue::Boolean(true)));
                }
                if identifier.eq_ignore_ascii_case("FALSE") {
                    return Ok(ValueExpr::Literal(QueryValue::Boolean(false)));
                }
                if self.consume_token(&Token::Dot) {
                    let field = self.parse_field_name()?;
                    if identifier.eq_ignore_ascii_case("EVENT") {
                        Ok(ValueExpr::Field(FieldRef::Event(
                            field.to_ascii_lowercase(),
                        )))
                    } else if identifier.eq_ignore_ascii_case("OBJECT") {
                        Ok(ValueExpr::Field(FieldRef::Object(
                            field.to_ascii_lowercase(),
                        )))
                    } else {
                        Err(OcelError::new(format!(
                            "unsupported field namespace '{identifier}', expected event or object"
                        )))
                    }
                } else {
                    Ok(ValueExpr::Literal(QueryValue::String(identifier)))
                }
            }
            other => Err(OcelError::new(format!(
                "expected field or literal in state query, found {other:?}"
            ))),
        }
    }

    fn parse_field_name(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) | Some(Token::String(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected field name in state query, found {other:?}"
            ))),
        }
    }

    fn parse_identifier(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected identifier in state query, found {other:?}"
            ))),
        }
    }

    fn parse_type_name(&mut self) -> OcelResult<String> {
        match self.next().cloned() {
            Some(Token::Identifier(identifier)) | Some(Token::String(identifier)) => Ok(identifier),
            other => Err(OcelError::new(format!(
                "expected leading object type name in state query, found {other:?}"
            ))),
        }
    }
}
