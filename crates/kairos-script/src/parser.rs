use crate::Diagnostic;
use crate::ast::*;
use crate::lexer::{Tok, strip_comment, tokenize};

struct Line {
    no: usize,
    indent: usize,
    text: String,
}

/// Parses one expression, such as the inside of a `[...]` interpolation in dialogue.
pub fn parse_expr(text: &str) -> Result<Expr, Diagnostic> {
    let mut c = Cursor::new(tokenize(text, 0)?, 0);
    let e = c.expr()?;
    c.end()?;
    Ok(e)
}

pub fn parse(src: &str) -> (Vec<Stmt>, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let lines: Vec<Line> = src
        .lines()
        .enumerate()
        .filter_map(|(i, raw)| {
            let code = strip_comment(raw).trim_end();
            let text = code.trim_start();
            if text.is_empty() {
                return None;
            }
            if code[..code.len() - text.len()].contains('\t') {
                diags.push(Diagnostic::new(i + 1, "tabs are not allowed in indentation"));
            }
            Some(Line { no: i + 1, indent: code.len() - text.len(), text: text.to_string() })
        })
        .collect();
    let mut p = Parser { lines, pos: 0, diags };
    let stmts = p.block(0);
    (stmts, p.diags)
}

struct Parser {
    lines: Vec<Line>,
    pos: usize,
    diags: Vec<Diagnostic>,
}

impl Parser {
    fn block(&mut self, indent: usize) -> Vec<Stmt> {
        let mut out = Vec::new();
        while let Some(line) = self.lines.get(self.pos) {
            if line.indent < indent {
                break;
            }
            if line.indent > indent {
                self.diags.push(Diagnostic::new(line.no, "unexpected indent"));
                self.pos += 1;
                continue;
            }
            if let Some(stmt) = self.stmt() {
                out.push(stmt);
            }
        }
        out
    }

    /// Parses the indented block that must follow a header line ending in `:`.
    fn child_block(&mut self, header_indent: usize, header_line: usize) -> Vec<Stmt> {
        match self.lines.get(self.pos) {
            Some(next) if next.indent > header_indent => {
                let indent = next.indent;
                self.block(indent)
            }
            _ => {
                self.diags.push(Diagnostic::new(header_line, "expected an indented block"));
                Vec::new()
            }
        }
    }

    fn child_lines(&mut self, header_indent: usize) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(line) = self.lines.get(self.pos) {
            if line.indent <= header_indent {
                break;
            }
            out.push(line.text.clone());
            self.pos += 1;
        }
        out
    }

    fn stmt(&mut self) -> Option<Stmt> {
        let line = &self.lines[self.pos];
        let (no, indent, source) = (line.no, line.indent, line.text.clone());
        self.pos += 1;
        let toks = match tokenize(&source, no) {
            Ok(t) => t,
            Err(d) => {
                self.diags.push(d);
                self.skip_children(indent);
                return None;
            }
        };
        match self.stmt_kind(Cursor::new(toks, no), indent, no) {
            Ok(kind) => Some(Stmt { line: no, source, kind }),
            Err(d) => {
                self.diags.push(d);
                self.skip_children(indent);
                None
            }
        }
    }

    fn skip_children(&mut self, indent: usize) {
        while self.lines.get(self.pos).is_some_and(|l| l.indent > indent) {
            self.pos += 1;
        }
    }

    fn stmt_kind(&mut self, mut c: Cursor, indent: usize, no: usize) -> Result<StmtKind, Diagnostic> {
        let kind = match c.peek().cloned() {
            Some(Tok::Op("$")) => {
                c.next();
                let name = c.ident()?;
                if c.peek() == Some(&Tok::Op("(")) {
                    let Some((name, method)) = name.rsplit_once('.') else {
                        return Err(c.err("expected an assignment like `$ x = 1` or a change like `$ xs.append(1)`"));
                    };
                    c.next();
                    let args = c.list_of(")", Cursor::expr)?;
                    c.end()?;
                    return Ok(StmtKind::Mutate { name: name.into(), method: method.into(), args });
                }
                let mut index = Vec::new();
                while c.peek() == Some(&Tok::Op("[")) {
                    c.next();
                    index.push(c.expr()?);
                    c.op("]")?;
                }
                let op = match c.next() {
                    Some(Tok::Op("=")) => AssignOp::Set,
                    Some(Tok::Op("+=")) => AssignOp::Add,
                    Some(Tok::Op("-=")) => AssignOp::Sub,
                    Some(Tok::Op("*=")) => AssignOp::Mul,
                    Some(Tok::Op("/=")) => AssignOp::Div,
                    _ => return Err(c.err("expected an assignment like `$ x = 1`")),
                };
                StmtKind::Assign { name, index, op, value: c.expr()? }
            }
            Some(Tok::Str(_)) | Some(Tok::Ident(_)) if is_say(&c) => say(&mut c)?,
            Some(Tok::Ident(kw)) => {
                c.next();
                match kw.as_str() {
                    "label" => {
                        let name = c.ident()?;
                        c.colon()?;
                        c.end()?;
                        return Ok(StmtKind::Label { name, body: self.child_block(indent, no) });
                    }
                    "menu" => {
                        c.colon()?;
                        c.end()?;
                        return Ok(self.menu(indent, no));
                    }
                    "if" => {
                        let cond = c.expr()?;
                        c.colon()?;
                        c.end()?;
                        return Ok(self.if_chain(cond, indent, no));
                    }
                    "transform" => {
                        let name = c.ident()?;
                        c.colon()?;
                        c.end()?;
                        return Ok(StmtKind::Transform { name, body: self.child_lines(indent) });
                    }
                    "jump" => StmtKind::Jump(c.ident()?),
                    "call" => StmtKind::Call(c.ident()?),
                    "return" => StmtKind::Return,
                    "open" => match c.ident()?.as_str() {
                        "load" => StmtKind::OpenLoad,
                        "settings" => StmtKind::OpenSettings,
                        screen => return Err(c.err(format!("unknown screen `{screen}`"))),
                    },
                    "quit" => StmtKind::Quit,
                    "pass" => StmtKind::Pass,
                    "character" => {
                        let id = c.ident()?;
                        let name = c.string()?;
                        let mut color = None;
                        let mut nvl = false;
                        while let Some(key) = c.opt_ident() {
                            if key == "nvl" {
                                nvl = true;
                                continue;
                            }
                            c.op("=")?;
                            let value = c.string()?;
                            match key.as_str() {
                                "color" => color = Some(value),
                                _ => return Err(c.err(format!("unknown character property `{key}`"))),
                            }
                        }
                        StmtKind::Character { id, name, color, nvl }
                    }
                    "define" | "default" => {
                        let name = c.ident()?;
                        c.op("=")?;
                        let value = c.expr()?;
                        if kw == "define" {
                            StmtKind::Define { name, value }
                        } else {
                            StmtKind::Default { name, value }
                        }
                    }
                    "scene" => {
                        let image = c.words();
                        let with = c.with_clause()?;
                        StmtKind::Scene { image: (!image.is_empty()).then_some(image), with }
                    }
                    "show" => {
                        if c.keyword("screen") {
                            StmtKind::ShowScreen(c.ident()?)
                        } else {
                            let image = c.words();
                            if image.is_empty() {
                                return Err(c.err("`show` needs an image name"));
                            }
                            let at = if c.keyword("at") { Some(c.ident()?) } else { None };
                            let with = c.with_clause()?;
                            StmtKind::Show { image, at, with }
                        }
                    }
                    "hide" => {
                        if c.keyword("screen") {
                            StmtKind::HideScreen(c.ident()?)
                        } else {
                            let tag = c.ident()?;
                            let with = c.with_clause()?;
                            StmtKind::Hide { tag, with }
                        }
                    }
                    "with" => StmtKind::With(c.ident()?),
                    "play" | "queue" => {
                        let channel = c.ident()?;
                        let asset = c.asset()?;
                        let mut looped = false;
                        let mut fadein = None;
                        while let Some(opt) = c.opt_ident() {
                            match opt.as_str() {
                                "loop" => looped = true,
                                "fadein" => fadein = Some(c.number()?),
                                _ => return Err(c.err(format!("unknown audio option `{opt}`"))),
                            }
                        }
                        StmtKind::Play { channel, asset, looped, queue: kw == "queue", fadein }
                    }
                    "voice" => {
                        let asset = c.asset()?;
                        StmtKind::Play { channel: "voice".into(), asset, looped: false, queue: false, fadein: None }
                    }
                    "stop" => {
                        let channel = c.ident()?;
                        let fadeout = if c.keyword("fadeout") { Some(c.number()?) } else { None };
                        StmtKind::Stop { channel, fadeout }
                    }
                    "pause" => StmtKind::Pause(if c.at_end() { None } else { Some(c.number()?) }),
                    "nvl" => StmtKind::Nvl(match c.ident()?.as_str() {
                        "on" => NvlAction::On,
                        "off" => NvlAction::Off,
                        "clear" => NvlAction::Clear,
                        action => return Err(c.err(format!("unknown NVL action `{action}`"))),
                    }),
                    "stage" => {
                        let tag = c.ident()?;
                        let props = stage_properties(&mut c)?;
                        StmtKind::StageSprite {
                            tag,
                            x: props.x,
                            y: props.y,
                            scale: props.scale,
                            rotation: props.rotation,
                            alpha: props.alpha,
                            over: props.over,
                            easing: props.easing,
                        }
                    }
                    "camera" => {
                        let props = stage_properties(&mut c)?;
                        StmtKind::Camera {
                            x: props.x,
                            y: props.y,
                            zoom: props.scale,
                            over: props.over,
                            easing: props.easing,
                        }
                    }
                    "shake" => StmtKind::ScreenShake { duration: c.number()?, amount: c.number()? },
                    "flash" => StmtKind::ScreenFlash { color: c.string()?, duration: c.number()? },
                    "elif" | "else" => return Err(c.err(format!("`{kw}` without a matching `if`"))),
                    _ => return Err(c.err(format!("unknown statement `{kw}`"))),
                }
            }
            _ => return Err(c.err("expected a statement")),
        };
        c.end()?;
        Ok(kind)
    }

    fn menu(&mut self, indent: usize, no: usize) -> StmtKind {
        let mut caption = None;
        let mut choices = Vec::new();
        let Some(child_indent) = self.lines.get(self.pos).filter(|l| l.indent > indent).map(|l| l.indent) else {
            self.diags.push(Diagnostic::new(no, "expected an indented block"));
            return StmtKind::Menu { caption, choices };
        };
        while let Some(line) = self.lines.get(self.pos).filter(|l| l.indent == child_indent) {
            let (cno, source) = (line.no, line.text.clone());
            self.pos += 1;
            let parsed = tokenize(&source, cno).and_then(|toks| {
                let mut c = Cursor::new(toks, cno);
                if !source.ends_with(':') {
                    let StmtKind::Say { speaker, text } = say(&mut c)? else { unreachable!() };
                    c.end()?;
                    return Ok(Err((speaker, text)));
                }
                let text = c.string()?;
                let guard = if c.keyword("if") { Some(c.expr()?) } else { None };
                c.colon()?;
                c.end()?;
                Ok(Ok((text, guard)))
            });
            match parsed {
                Ok(Ok((text, guard))) => {
                    let body = self.child_block(child_indent, cno);
                    choices.push(Choice { line: cno, source, text, guard, body });
                }
                Ok(Err(cap)) if caption.is_none() && choices.is_empty() => caption = Some(cap),
                Ok(Err(_)) => self.diags.push(Diagnostic::new(cno, "a menu caption must come before its choices")),
                Err(d) => {
                    self.diags.push(d);
                    self.skip_children(child_indent);
                }
            }
        }
        if choices.is_empty() {
            self.diags.push(Diagnostic::new(no, "menu has no choices"));
        }
        StmtKind::Menu { caption, choices }
    }

    fn if_chain(&mut self, cond: Expr, indent: usize, no: usize) -> StmtKind {
        let mut branches = vec![Branch { cond: Some(cond), body: self.child_block(indent, no) }];
        while let Some(line) = self.lines.get(self.pos).filter(|l| l.indent == indent) {
            let is_elif = line.text.starts_with("elif ");
            let is_else = line.text.starts_with("else") && line.text[4..].trim() == ":";
            if !is_elif && !is_else {
                break;
            }
            let (bno, text) = (line.no, line.text.clone());
            self.pos += 1;
            let cond = if is_else {
                None
            } else {
                let parsed = tokenize(&text, bno).and_then(|toks| {
                    let mut c = Cursor::new(toks, bno);
                    c.next();
                    let e = c.expr()?;
                    c.colon()?;
                    c.end()?;
                    Ok(e)
                });
                match parsed {
                    Ok(e) => Some(e),
                    Err(d) => {
                        self.diags.push(d);
                        self.skip_children(indent);
                        continue;
                    }
                }
            };
            branches.push(Branch { cond, body: self.child_block(indent, bno) });
            if is_else {
                break;
            }
        }
        StmtKind::If(branches)
    }
}

struct StageProperties {
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
    rotation: Option<f64>,
    alpha: Option<f64>,
    over: Option<f64>,
    easing: Easing,
}

fn stage_properties(c: &mut Cursor) -> Result<StageProperties, Diagnostic> {
    let (mut x, mut y, mut scale, mut rotation, mut alpha, mut over) = (None, None, None, None, None, None);
    let mut easing = Easing::Linear;
    while let Some(key) = c.opt_ident() {
        match key.as_str() {
            "at" => easing = Easing::At,
            "linear" => easing = Easing::Linear,
            "ease" => easing = Easing::Ease,
            "x" | "y" | "scale" | "rotation" | "alpha" | "zoom" | "over" => {
                c.op("=")?;
                let value = c.number()?;
                match key.as_str() {
                    "x" => x = Some(value),
                    "y" => y = Some(value),
                    "scale" | "zoom" => scale = Some(value),
                    "rotation" => rotation = Some(value),
                    "alpha" => alpha = Some(value),
                    "over" if value >= 0.0 => over = Some(value),
                    "over" => return Err(c.err("timeline duration cannot be negative")),
                    _ => unreachable!(),
                }
            }
            _ => return Err(c.err(format!("unknown staging property `{key}`"))),
        }
    }
    Ok(StageProperties { x, y, scale, rotation, alpha, over, easing })
}

fn is_say(c: &Cursor) -> bool {
    matches!(
        (c.toks.first(), c.toks.get(1)),
        (Some(Tok::Str(_)), None | Some(Tok::Str(_))) | (Some(Tok::Ident(_)), Some(Tok::Str(_)))
    ) && c.toks.len() <= 2
}

fn say(c: &mut Cursor) -> Result<StmtKind, Diagnostic> {
    match (c.next(), c.next()) {
        (Some(Tok::Str(text)), None) => Ok(StmtKind::Say { speaker: None, text }),
        (Some(Tok::Str(name)), Some(Tok::Str(text))) => Ok(StmtKind::Say { speaker: Some(Speaker::Name(name)), text }),
        (Some(Tok::Ident(id)), Some(Tok::Str(text))) => {
            Ok(StmtKind::Say { speaker: Some(Speaker::Character(id)), text })
        }
        _ => Err(c.err("expected dialogue like `e \"Hello\"`")),
    }
}

struct Cursor {
    toks: Vec<Tok>,
    pos: usize,
    line: usize,
}

const CLAUSE_KEYWORDS: [&str; 5] = ["at", "with", "as", "behind", "onlayer"];

impl Cursor {
    fn new(toks: Vec<Tok>, line: usize) -> Self {
        Cursor { toks, pos: 0, line }
    }

    fn err(&self, msg: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.line, msg)
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    fn end(&self) -> Result<(), Diagnostic> {
        match self.peek() {
            None => Ok(()),
            Some(t) => Err(self.err(format!("unexpected {}", describe(t)))),
        }
    }

    fn op(&mut self, op: &str) -> Result<(), Diagnostic> {
        match self.next() {
            Some(Tok::Op(o)) if o == op => Ok(()),
            _ => Err(self.err(format!("expected `{op}`"))),
        }
    }

    fn colon(&mut self) -> Result<(), Diagnostic> {
        self.op(":")
    }

    fn ident(&mut self) -> Result<String, Diagnostic> {
        match self.next() {
            Some(Tok::Ident(s)) => Ok(s),
            _ => Err(self.err("expected a name")),
        }
    }

    fn opt_ident(&mut self) -> Option<String> {
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.pos += 1;
                Some(s)
            }
            _ => None,
        }
    }

    fn keyword(&mut self, kw: &str) -> bool {
        let hit = matches!(self.peek(), Some(Tok::Ident(s)) if s == kw);
        if hit {
            self.pos += 1;
        }
        hit
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        match self.next() {
            Some(Tok::Str(s)) => Ok(s),
            _ => Err(self.err("expected a string")),
        }
    }

    fn asset(&mut self) -> Result<String, Diagnostic> {
        match self.next() {
            Some(Tok::Str(s) | Tok::Ident(s)) => Ok(s),
            _ => Err(self.err("expected an audio file or name")),
        }
    }

    fn number(&mut self) -> Result<f64, Diagnostic> {
        match self.next() {
            Some(Tok::Int(n)) => Ok(n as f64),
            Some(Tok::Float(n)) => Ok(n),
            _ => Err(self.err("expected a number")),
        }
    }

    fn words(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(Tok::Ident(w)) = self.peek() {
            if CLAUSE_KEYWORDS.contains(&w.as_str()) {
                break;
            }
            out.push(w.clone());
            self.pos += 1;
        }
        out
    }

    fn with_clause(&mut self) -> Result<Option<String>, Diagnostic> {
        if self.keyword("with") { self.ident().map(Some) } else { Ok(None) }
    }

    fn expr(&mut self) -> Result<Expr, Diagnostic> {
        self.binary(0)
    }

    fn binary(&mut self, min_prec: u8) -> Result<Expr, Diagnostic> {
        let mut lhs = self.unary()?;
        while let Some((op, prec)) = self.peek_binop() {
            if prec < min_prec {
                break;
            }
            self.pos += if op == BinOp::NotIn { 2 } else { 1 };
            let rhs = self.binary(prec + 1)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn peek_binop(&self) -> Option<(BinOp, u8)> {
        let not_in = matches!(self.peek(), Some(Tok::Ident(s)) if s == "not")
            && matches!(self.toks.get(self.pos + 1), Some(Tok::Ident(s)) if s == "in");
        if not_in { Some((BinOp::NotIn, 4)) } else { self.peek().and_then(binop) }
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        match self.peek() {
            Some(Tok::Op("-")) => {
                self.pos += 1;
                Ok(Expr::Unary(UnOp::Neg, Box::new(self.unary()?)))
            }
            Some(Tok::Ident(s)) if s == "not" => {
                self.pos += 1;
                // `not` binds looser than comparisons, as in Python.
                Ok(Expr::Unary(UnOp::Not, Box::new(self.binary(3)?)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let mut e = self.atom()?;
        while self.peek() == Some(&Tok::Op("[")) {
            self.pos += 1;
            let index = self.expr()?;
            self.op("]")?;
            e = Expr::Index(Box::new(e), Box::new(index));
        }
        Ok(e)
    }

    fn atom(&mut self) -> Result<Expr, Diagnostic> {
        match self.next() {
            Some(Tok::Int(n)) => Ok(Expr::Lit(Value::Int(n))),
            Some(Tok::Float(n)) => Ok(Expr::Lit(Value::Float(n))),
            Some(Tok::Str(s)) => Ok(Expr::Lit(Value::Str(s))),
            Some(Tok::Ident(s)) if self.peek() == Some(&Tok::Op("(")) => {
                self.pos += 1;
                let args = self.list_of(")", Self::expr)?;
                Ok(match s.rsplit_once('.') {
                    Some((receiver, method)) => Expr::Method(Box::new(Expr::Var(receiver.into())), method.into(), args),
                    None => Expr::Call(s, args),
                })
            }
            Some(Tok::Ident(s)) => Ok(match s.as_str() {
                "True" | "true" => Expr::Lit(Value::Bool(true)),
                "False" | "false" => Expr::Lit(Value::Bool(false)),
                "None" | "none" => Expr::Lit(Value::None),
                _ => Expr::Var(s),
            }),
            Some(Tok::Op("(")) => {
                let e = self.expr()?;
                self.op(")")?;
                Ok(e)
            }
            Some(Tok::Op("[")) => Ok(Expr::List(self.list_of("]", Self::expr)?)),
            Some(Tok::Op("{")) => Ok(Expr::Dict(self.list_of("}", |c| {
                let key = c.expr()?;
                c.colon()?;
                Ok((key, c.expr()?))
            })?)),
            Some(t) => Err(self.err(format!("unexpected {} in expression", describe(&t)))),
            None => Err(self.err("expected an expression")),
        }
    }

    /// Comma-separated items up to `close`, allowing a trailing comma.
    fn list_of<T>(
        &mut self,
        close: &'static str,
        mut item: impl FnMut(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<Vec<T>, Diagnostic> {
        let mut items = Vec::new();
        loop {
            if self.peek() == Some(&Tok::Op(close)) {
                self.pos += 1;
                return Ok(items);
            }
            items.push(item(self)?);
            match self.next() {
                Some(Tok::Op(",")) => {}
                Some(Tok::Op(op)) if op == close => return Ok(items),
                _ => return Err(self.err(format!("expected `,` or `{close}`"))),
            }
        }
    }
}

fn binop(t: &Tok) -> Option<(BinOp, u8)> {
    Some(match t {
        Tok::Ident(s) if s == "or" => (BinOp::Or, 1),
        Tok::Ident(s) if s == "and" => (BinOp::And, 2),
        Tok::Ident(s) if s == "in" => (BinOp::In, 4),
        Tok::Op("==") => (BinOp::Eq, 4),
        Tok::Op("!=") => (BinOp::Ne, 4),
        Tok::Op("<") => (BinOp::Lt, 4),
        Tok::Op("<=") => (BinOp::Le, 4),
        Tok::Op(">") => (BinOp::Gt, 4),
        Tok::Op(">=") => (BinOp::Ge, 4),
        Tok::Op("+") => (BinOp::Add, 5),
        Tok::Op("-") => (BinOp::Sub, 5),
        Tok::Op("*") => (BinOp::Mul, 6),
        Tok::Op("/") => (BinOp::Div, 6),
        Tok::Op("%") => (BinOp::Rem, 6),
        _ => return None,
    })
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(s) => format!("`{s}`"),
        Tok::Int(n) => format!("`{n}`"),
        Tok::Float(n) => format!("`{n}`"),
        Tok::Str(s) => format!("string \"{s}\""),
        Tok::Op(o) => format!("`{o}`"),
    }
}
