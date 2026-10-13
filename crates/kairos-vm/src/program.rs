use std::collections::{BTreeMap, HashMap};

use kairos_script::{AssignOp, Diagnostic, Expr, Speaker, Stmt, StmtKind};

#[derive(Debug, Clone, PartialEq)]
pub struct MenuChoice {
    pub text: String,
    pub guard: Option<Expr>,
    pub target: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    Say { speaker: Option<Speaker>, text: String },
    Menu { caption: Option<(Option<Speaker>, String)>, choices: Vec<MenuChoice> },
    Pause(Option<f64>),
    Goto(usize),
    GotoUnless(Expr, usize),
    Jump(String),
    Call(String),
    Return,
    OpenLoad,
    OpenSettings,
    Quit,
    Assign { name: String, op: AssignOp, value: Expr },
    Scene { image: Option<Vec<String>>, with: Option<String> },
    Show { image: Vec<String>, at: Option<String>, with: Option<String> },
    Hide { tag: String, with: Option<String> },
    With(String),
    Play { channel: String, asset: String, looped: bool, queue: bool, fadein: Option<f64> },
    Stop { channel: String, fadeout: Option<f64> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub name: String,
    pub color: Option<String>,
}

/// A script compiled to a flat instruction list.
///
/// Every instruction carries a stable ID derived from its enclosing label, its source text and
/// its occurrence count, so saves keep pointing at the same line when unrelated lines change.
#[derive(Debug, Default)]
pub struct Program {
    pub instrs: Vec<Instr>,
    pub ids: Vec<String>,
    pub lines: Vec<usize>,
    /// Index into the file list passed to [`compile`], per instruction.
    pub files: Vec<usize>,
    pub labels: HashMap<String, usize>,
    pub characters: HashMap<String, Character>,
    pub defines: Vec<(String, Expr)>,
    pub defaults: Vec<(String, Expr)>,
    pub transforms: HashMap<String, Vec<String>>,
    id_index: HashMap<String, usize>,
}

impl Program {
    pub fn pc_of(&self, id: &str) -> Option<usize> {
        self.id_index.get(id).copied()
    }

    pub fn label_at(&self, pc: usize) -> Option<&str> {
        self.labels
            .iter()
            .filter(|(_, start)| **start <= pc)
            .max_by_key(|(_, start)| *start)
            .map(|(name, _)| name.as_str())
    }

    /// The first instruction at or after `line`, for "play from this line".
    pub fn pc_at_line(&self, line: usize) -> Option<usize> {
        self.lines.iter().position(|&l| l >= line)
    }

    /// The first instruction at or after `line` in file `file` (an index into the list given to [`compile`]).
    pub fn pc_at(&self, file: usize, line: usize) -> Option<usize> {
        (0..self.instrs.len()).find(|&pc| self.files[pc] == file && self.lines[pc] >= line && self.lines[pc] > 0)
    }
}

/// Compiles parsed files in order. Labels fall through into the next label, as in Ren'Py.
/// Diagnostics are paired with the index of the file they belong to.
pub fn compile<S: AsRef<[Stmt]>>(files: &[S]) -> (Program, Vec<(usize, Diagnostic)>) {
    let mut c = Compiler::default();
    for (file, stmts) in files.iter().enumerate() {
        c.file = file;
        for stmt in stmts.as_ref() {
            c.top_level(stmt);
        }
    }
    c.emit(Instr::Return, 0, "#end");
    for (pc, instr) in c.program.instrs.iter().enumerate() {
        if let Instr::Jump(l) | Instr::Call(l) = instr
            && !c.program.labels.contains_key(l)
        {
            c.diags.push((c.program.files[pc], Diagnostic::new(c.program.lines[pc], format!("unknown label `{l}`"))));
        }
        if let Instr::Say { speaker: Some(Speaker::Character(id)), .. } = instr
            && !c.program.characters.contains_key(id)
        {
            c.diags
                .push((c.program.files[pc], Diagnostic::new(c.program.lines[pc], format!("unknown character `{id}`"))));
        }
    }
    let mut program = c.program;
    program.id_index = program.ids.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();
    (program, c.diags)
}

#[derive(Default)]
struct Compiler {
    program: Program,
    diags: Vec<(usize, Diagnostic)>,
    file: usize,
    label: String,
    occurrences: BTreeMap<(String, u64), usize>,
}

impl Compiler {
    fn emit(&mut self, instr: Instr, line: usize, source: &str) -> usize {
        let hash = fnv1a(source);
        let n = self.occurrences.entry((self.label.clone(), hash)).or_default();
        self.program.ids.push(format!("{}:{hash:016x}:{n}", self.label));
        *n += 1;
        self.program.instrs.push(instr);
        self.program.lines.push(line);
        self.program.files.push(self.file);
        self.program.instrs.len() - 1
    }

    fn patch(&mut self, pc: usize, target: usize) {
        match &mut self.program.instrs[pc] {
            Instr::Goto(t) | Instr::GotoUnless(_, t) => *t = target,
            _ => unreachable!("patched a non-jump"),
        }
    }

    fn here(&self) -> usize {
        self.program.instrs.len()
    }

    fn top_level(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Label { .. }
            | StmtKind::Character { .. }
            | StmtKind::Define { .. }
            | StmtKind::Default { .. }
            | StmtKind::Transform { .. } => self.stmt(stmt),
            _ => self.diags.push((self.file, Diagnostic::new(stmt.line, "statements must be inside a label"))),
        }
    }

    fn block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        let (line, src) = (stmt.line, stmt.source.as_str());
        let simple = match &stmt.kind {
            StmtKind::Label { name, body } => {
                let pc = self.here();
                if self.program.labels.insert(name.clone(), pc).is_some() {
                    self.diags.push((self.file, Diagnostic::new(line, format!("label `{name}` is defined twice"))));
                }
                let outer = std::mem::replace(&mut self.label, name.clone());
                self.block(body);
                if name == "splashscreen" {
                    self.emit(Instr::Return, line, "#splashscreen end");
                }
                self.label = outer;
                return;
            }
            StmtKind::Character { id, name, color } => {
                let def = Character { name: name.clone(), color: color.clone() };
                self.program.characters.insert(id.clone(), def);
                return;
            }
            StmtKind::Define { name, value } => {
                self.program.defines.push((name.clone(), value.clone()));
                return;
            }
            StmtKind::Default { name, value } => {
                self.program.defaults.push((name.clone(), value.clone()));
                return;
            }
            StmtKind::Transform { name, body } => {
                self.program.transforms.insert(name.clone(), body.clone());
                return;
            }
            StmtKind::Pass => return,
            StmtKind::Menu { caption, choices } => {
                let menu = self.emit(Instr::Menu { caption: caption.clone(), choices: Vec::new() }, line, src);
                let mut compiled = Vec::new();
                let mut exits = Vec::new();
                for choice in choices {
                    compiled.push(MenuChoice {
                        text: choice.text.clone(),
                        guard: choice.guard.clone(),
                        target: self.here(),
                    });
                    self.block(&choice.body);
                    exits.push(self.emit(Instr::Goto(0), choice.line, &format!("#exit {}", choice.source)));
                }
                let end = self.here();
                for pc in exits {
                    self.patch(pc, end);
                }
                if let Instr::Menu { choices, .. } = &mut self.program.instrs[menu] {
                    *choices = compiled;
                }
                return;
            }
            StmtKind::If(branches) => {
                let mut exits = Vec::new();
                for (i, branch) in branches.iter().enumerate() {
                    let tag = format!("#branch{i} {src}");
                    let test = branch.cond.as_ref().map(|c| self.emit(Instr::GotoUnless(c.clone(), 0), line, &tag));
                    self.block(&branch.body);
                    exits.push(self.emit(Instr::Goto(0), line, &format!("#exit{i} {src}")));
                    if let Some(test) = test {
                        let next = self.here();
                        self.patch(test, next);
                    }
                }
                let end = self.here();
                for pc in exits {
                    self.patch(pc, end);
                }
                return;
            }
            StmtKind::Jump(l) => Instr::Jump(l.clone()),
            StmtKind::Call(l) => Instr::Call(l.clone()),
            StmtKind::Return => Instr::Return,
            StmtKind::OpenLoad => Instr::OpenLoad,
            StmtKind::OpenSettings => Instr::OpenSettings,
            StmtKind::Quit => Instr::Quit,
            StmtKind::Say { speaker, text } => Instr::Say { speaker: speaker.clone(), text: text.clone() },
            StmtKind::Assign { name, op, value } => Instr::Assign { name: name.clone(), op: *op, value: value.clone() },
            StmtKind::Scene { image, with } => Instr::Scene { image: image.clone(), with: with.clone() },
            StmtKind::Show { image, at, with } => {
                Instr::Show { image: image.clone(), at: at.clone(), with: with.clone() }
            }
            StmtKind::Hide { tag, with } => Instr::Hide { tag: tag.clone(), with: with.clone() },
            StmtKind::With(t) => Instr::With(t.clone()),
            StmtKind::Play { channel, asset, looped, queue, fadein } => Instr::Play {
                channel: channel.clone(),
                asset: asset.clone(),
                looped: *looped,
                queue: *queue,
                fadein: *fadein,
            },
            StmtKind::Stop { channel, fadeout } => Instr::Stop { channel: channel.clone(), fadeout: *fadeout },
            StmtKind::Pause(d) => Instr::Pause(*d),
        };
        self.emit(simple, line, src);
    }
}

fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}
