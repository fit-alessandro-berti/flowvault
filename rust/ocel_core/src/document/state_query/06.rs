
impl QueryParser {

    fn expect_keyword(&mut self, keyword: &str) -> OcelResult<()> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(OcelError::new(format!(
                "expected keyword {keyword} in state query"
            )))
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        match self.tokens.get(self.position) {
            Some(Token::Identifier(identifier)) if identifier.eq_ignore_ascii_case(keyword) => {
                self.position += 1;
                true
            }
            _ => false,
        }
    }

    fn expect_token(&mut self, token: &Token) -> OcelResult<()> {
        if self.consume_token(token) {
            Ok(())
        } else {
            Err(OcelError::new(format!(
                "expected token {token:?} in state query"
            )))
        }
    }

    fn consume_token(&mut self, token: &Token) -> bool {
        if self.tokens.get(self.position) == Some(token) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    fn is_done(&self) -> bool {
        self.position >= self.tokens.len()
    }
}
