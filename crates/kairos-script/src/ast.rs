use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    /// Keys are strings so a dictionary maps onto JSON for the editor's debugger.
    Dict(BTreeMap<String, Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    In,
    NotIn,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Lit(Value),
    Var(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    List(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),
    Index(Box<Expr>, Box<Expr>),
    /// A built-in function such as `len(xs)`.
    Call(String, Vec<Expr>),
    /// A method on a variable, such as `affection.get("kai", 0)`.
    Method(Box<Expr>, String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    /// 1-based source line.
    pub line: usize,
    /// Trimmed source text of the statement's first line; the basis of stable statement IDs.
    pub source: String,
    pub kind: StmtKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Speaker {
    Character(String),
    Name(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub line: usize,
    pub source: String,
    pub text: String,
    pub guard: Option<Expr>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub cond: Option<Expr>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Label {
        name: String,
        body: Vec<Stmt>,
    },
    Jump(String),
    Call(String),
    Return,
    OpenLoad,
    OpenSettings,
    Quit,
    Pass,
    Say {
        speaker: Option<Speaker>,
        text: String,
    },
    Character {
        id: String,
        name: String,
        color: Option<String>,
    },
    Menu {
        caption: Option<(Option<Speaker>, String)>,
        choices: Vec<Choice>,
    },
    If(Vec<Branch>),
    Define {
        name: String,
        value: Expr,
    },
    Default {
        name: String,
        value: Expr,
    },
    /// `$ name[i][j] op= value`; `index` is empty for a plain variable.
    Assign {
        name: String,
        index: Vec<Expr>,
        op: AssignOp,
        value: Expr,
    },
    /// `$ name.method(args)`, such as `$ inventory.append("key")`.
    Mutate {
        name: String,
        method: String,
        args: Vec<Expr>,
    },
    Scene {
        image: Option<Vec<String>>,
        with: Option<String>,
    },
    Show {
        image: Vec<String>,
        at: Option<String>,
        with: Option<String>,
    },
    Hide {
        tag: String,
        with: Option<String>,
    },
    With(String),
    Play {
        channel: String,
        asset: String,
        looped: bool,
        queue: bool,
        fadein: Option<f64>,
    },
    Stop {
        channel: String,
        fadeout: Option<f64>,
    },
    Pause(Option<f64>),
    Transform {
        name: String,
        body: Vec<String>,
    },
}
