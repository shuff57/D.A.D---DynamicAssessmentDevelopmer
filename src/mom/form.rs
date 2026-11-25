//! The Write screen's guided form: a teacher picks a course, a topic, the kind of answer and a few
//! options from lists, and [`Form::request`] turns that into the request the AI writer gets. Also the
//! "change it" requests for a question that already exists, and plain-language progress messages.
//! Pure logic, so it is tested without a window.

use serde::{Deserialize, Serialize};

/// Courses and their common topics. The last course, "Other", has no topics: the teacher types one.
pub const COURSES: &[(&str, &[&str])] = &[
    ("Integrated Math 1", &["Functions and relations", "Solving linear equations", "Linear functions and graphs",
        "Linear inequalities", "Systems of equations", "Exponential functions", "Sequences",
        "Data displays and statistics", "Linear regression", "Transformations", "Congruence", "Coordinate geometry"]),
    ("Integrated Math 2", &["Quadratic functions", "Factoring", "Solving quadratic equations", "Exponents and radicals",
        "Probability", "Similarity", "Right triangle trigonometry", "Circles"]),
    ("Integrated Math 3", &["Polynomial functions", "Rational functions", "Exponential and logarithmic functions",
        "Trigonometric functions", "Sequences and series", "Normal distribution", "Sampling and inference"]),
    ("Algebra 1", &["Expressions", "Solving linear equations", "Linear inequalities", "Linear functions", "Systems of equations",
        "Exponent rules", "Polynomials", "Factoring", "Quadratic equations"]),
    ("Geometry", &["Angles and lines", "Triangles", "Congruence", "Similarity", "Right triangles and trigonometry",
        "Area and perimeter", "Volume and surface area", "Circles", "Coordinate geometry"]),
    ("Algebra 2", &["Functions and transformations", "Quadratics and complex numbers", "Polynomials", "Rational expressions",
        "Radicals", "Exponential and logarithmic functions", "Sequences and series", "Trigonometry", "Probability"]),
    ("Precalculus", &["Functions", "Polynomial and rational functions", "Exponential and logarithmic functions",
        "Trigonometric functions", "Trigonometric identities", "Vectors", "Matrices", "Conic sections", "Limits"]),
    ("Statistics", &["Data collection and sampling", "Data displays", "Mean, median and spread", "Probability",
        "Counting and combinations", "Normal distribution", "Sampling distributions", "Confidence intervals",
        "Hypothesis tests", "Regression and correlation", "Chi-square tests"]),
    ("Finite Math", &["Sets", "Logic", "Counting", "Probability", "Simple and compound interest", "Annuities and loans",
        "Linear programming", "Matrices", "Systems of equations"]),
    ("Calculus", &["Limits", "Derivatives", "Derivative rules", "Applications of derivatives", "Integrals",
        "Integration techniques", "Applications of integrals"]),
    ("Other", &[]),
];

/// The first entry of every dropdown: leave the choice to the AI.
pub const AI_CHOOSES: &str = "Let the AI choose";

/// What a student enters for one part, in a teacher's words.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Answer {
    #[default]
    Number,
    Choice,
    SelectAll,
    Dropdown,
    Expression,
    Exact,
    Interval,
    Written,
    Matching,
    Text,
    Complex,
    CalculatedComplex,
    Tuple,
    CalculatedTuple,
    Matrix,
    CalculatedMatrix,
    CalculatedInterval,
    Drawing,
    File,
    ComplexTuple,
    CalculatedComplexTuple,
    AlgebraicTuple,
    ComplexMatrix,
    CalculatedComplexMatrix,
    AlgebraicMatrix,
    ChemicalEquation,
    Molecule,
}

impl Answer {
    pub const ALL: [Answer; 27] = [
        Self::Number, Self::Exact, Self::Expression, Self::Choice, Self::SelectAll, Self::Dropdown,
        Self::Matching, Self::Interval, Self::CalculatedInterval, Self::Text, Self::Written,
        Self::Complex, Self::CalculatedComplex, Self::Tuple, Self::CalculatedTuple, Self::Matrix,
        Self::CalculatedMatrix, Self::Drawing, Self::File, Self::ComplexTuple, Self::CalculatedComplexTuple,
        Self::AlgebraicTuple, Self::ComplexMatrix, Self::CalculatedComplexMatrix, Self::AlgebraicMatrix,
        Self::ChemicalEquation, Self::Molecule,
    ];

    /// The answer types most questions use, shown first; the rest sit under "More question types".
    pub const COMMON: [Answer; 8] = [
        Self::Number, Self::Exact, Self::Expression, Self::Choice, Self::SelectAll, Self::Text, Self::Written, Self::Drawing,
    ];

    /// The MyOpenMath code, student-facing format, and short explanation.
    pub(crate) fn details(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Number => ("number", "Number", "Enter an integer, decimal, or scientific notation. MyOpenMath grades within a tolerance."),
            Self::Exact => ("calculated", "Calculated number", "Enter a number or calculation, such as 0.75, 3/4, or √2. An answer format can require a fraction."),
            Self::Expression => ("numfunc", "Algebraic expression", "Enter an equivalent expression or function, such as x/2 or 0.5x when the format allows either."),
            Self::Choice => ("choices", "Multiple choice", "Choose one correct option."),
            Self::SelectAll => ("multans", "Multiple select", "Choose all correct options from the list."),
            Self::Dropdown => ("choices", "Dropdown", "Choose one answer from an inline menu."),
            Self::Matching => ("matching", "Matching", "Pair each item with its matching option."),
            Self::Interval => ("interval", "Interval", "Enter an interval with numeric endpoints, such as (2, 5]."),
            Self::CalculatedInterval => ("calcinterval", "Calculated interval", "Enter an interval with calculated endpoints, such as [1/2, √3]."),
            Self::Text => ("string", "Fill in the blank", "Type a word or phrase in an inline blank. MyOpenMath checks the accepted text answers."),
            Self::Written => ("essay", "Written response", "Write an explanation for the teacher to grade."),
            Self::Complex => ("complex", "Complex number", "Enter a complex number, such as 2 + 3i."),
            Self::CalculatedComplex => ("calccomplex", "Calculated complex number", "Enter a complex number with calculated components, such as 1/2 + √2i."),
            Self::Tuple => ("ntuple", "Ordered tuple", "Enter an ordered list of numbers, such as (2, 5)."),
            Self::CalculatedTuple => ("calcntuple", "Calculated tuple", "Enter an ordered list with entries such as fractions or radicals."),
            Self::Matrix => ("matrix", "Matrix", "Enter numeric values in a matrix."),
            Self::CalculatedMatrix => ("calcmatrix", "Calculated matrix", "Enter matrix entries that may contain calculations."),
            Self::Drawing => ("draw", "Graphing or drawing", "Plot points, lines, or curves with the drawing tool. MyOpenMath grades the drawing."),
            Self::File => ("file", "File upload", "Upload a file for the teacher to review."),
            Self::ComplexTuple => ("complexntuple", "Complex tuple", "Enter an ordered list of complex numbers."),
            Self::CalculatedComplexTuple => ("calccomplexntuple", "Calculated complex tuple", "Enter complex tuple entries using calculations."),
            Self::AlgebraicTuple => ("algntuple", "Algebraic tuple", "Enter an ordered list of algebraic expressions."),
            Self::ComplexMatrix => ("complexmatrix", "Complex matrix", "Enter a matrix of complex numbers."),
            Self::CalculatedComplexMatrix => ("calccomplexmatrix", "Calculated complex matrix", "Enter complex matrix entries using calculations."),
            Self::AlgebraicMatrix => ("algmatrix", "Algebraic matrix", "Enter a matrix of algebraic expressions."),
            Self::ChemicalEquation => ("chemeqn", "Chemical equation", "Enter a chemical equation with formulas and coefficients."),
            Self::Molecule => ("molecule", "Chemical molecule", "Draw a molecular structure in the molecule editor."),
        }
    }

    /// What the writer is told, naming the IMathAS answer type so it never has to guess.
    fn spec(self, f: &Form) -> String {
        let n = match f.options { Some(n) => format!("{n} options"), None => "3 to 5 options (you choose how many)".into() };
        match self {
            Answer::Number => format!("the student types a number ({}; IMathAS type number)", rounding_spec(f.rounding)),
            Answer::Choice => format!("multiple choice with {n} and exactly one right answer (IMathAS type choices)"),
            Answer::SelectAll => format!("select all that apply, {n}, at least one right (IMathAS type multans)"),
            Answer::Dropdown => format!("a dropdown inside the sentence with {n} (IMathAS type choices shown as a select)"),
            Answer::Expression => "the student types an algebraic expression or equation (IMathAS type numfunc)".into(),
            Answer::Exact => "the student types an exact value such as a fraction or a square root (IMathAS type calculated; accept equivalent decimals unless a specific format is requested)".into(),
            Answer::Interval => "the student types an interval (IMathAS type interval)".into(),
            Answer::Text => "a fill-in-the-blank word or phrase using IMathAS type string with the answer box inline".into(),
            Answer::Written => "a written explanation the teacher grades by hand (IMathAS type essay)".into(),
            other => {
                let (code, _, description) = other.details();
                format!("IMathAS type {code}. Student instructions: {description}")
            }
        }
    }

    fn has_options(self) -> bool { matches!(self, Answer::Choice | Answer::SelectAll | Answer::Dropdown) }
}

/// One part's answer as the writer is told it; None leaves it to the AI.
fn answer_spec(a: Option<Answer>, f: &Form) -> String {
    match a {
        Some(a) => a.spec(f),
        None => "an answer type you choose to fit the question (a number, multiple choice, an expression, ...)".into(),
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Difficulty { Easy, #[default] Medium, Hard }

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard];
    pub fn label(self) -> &'static str {
        match self { Difficulty::Easy => "Easy", Difficulty::Medium => "Medium", Difficulty::Hard => "Hard" }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Rounding { Whole, One, #[default] Two, Three, Four, Exact }

impl Rounding {
    pub const ALL: [Rounding; 6] = [Rounding::Whole, Rounding::One, Rounding::Two, Rounding::Three, Rounding::Four, Rounding::Exact];
    pub fn label(self) -> &'static str {
        match self {
            Rounding::Whole => "Whole number",
            Rounding::One => "1 decimal place",
            Rounding::Two => "2 decimal places",
            Rounding::Three => "3 decimal places",
            Rounding::Four => "4 decimal places",
            Rounding::Exact => "No rounding (exact)",
        }
    }
}

fn rounding_spec(r: Option<Rounding>) -> &'static str {
    match r {
        Some(Rounding::Whole) => "rounded to a whole number; say so in the question",
        Some(Rounding::One) => "rounded to 1 decimal place; say so in the question",
        Some(Rounding::Two) => "rounded to 2 decimal places; say so in the question",
        Some(Rounding::Three) => "rounded to 3 decimal places; say so in the question",
        Some(Rounding::Four) => "rounded to 4 decimal places; say so in the question",
        Some(Rounding::Exact) => "the exact value, so choose numbers that make it come out exact",
        None => "if it is not a whole number, choose sensible rounding and say it in the question",
    }
}

/// The topic: the AI's pick, one from the course's list, or the teacher's own words.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Topic { Ai, Listed(usize), Typed }

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HelpStyle { #[default] DetailedSolution, WrittenHelp }

/// Everything the form asks; every None (and `Topic::Ai`) is "let the AI choose". Saved between
/// sessions, so a teacher's course and habits stay picked.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Form {
    pub course: Option<usize>,
    pub topic: Topic,
    pub topic_text: String,
    /// How many parts; None: the AI decides.
    pub part_count: Option<usize>,
    pub part_range: Option<[usize; 2]>,
    /// One answer per part; with `part_count` None, one entry that applies to every part.
    pub answers: Vec<Option<Answer>>,
    pub prefer_answer_types: bool,
    pub difficulty: Option<Difficulty>,
    pub rounding: Option<Rounding>,
    /// Options in multiple choice, select-all and dropdown parts.
    pub options: Option<usize>,
    /// A real-world situation: yes, no, or the AI's call.
    pub story: Option<bool>,
    pub random: bool,
    pub extra: String,
    pub figure: Option<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) line_variant: Option<super::line_variant::LineVariant>,
    pub(crate) help_style: HelpStyle,
}

impl Default for Form {
    fn default() -> Form {
        Form { course: None, topic: Topic::Ai, topic_text: String::new(), part_count: None, part_range: None, answers: vec![None], prefer_answer_types: false,
            difficulty: None, rounding: None, options: None, story: None, random: true, extra: String::new(),
            figure: None, line_variant: None, help_style: HelpStyle::default() }
    }
}

/// Most parts the form offers.
pub const MAX_PARTS: usize = 5;

impl Form {
    pub fn course_name(&self) -> Option<&'static str> { self.course.and_then(|i| COURSES.get(i)).map(|c| c.0) }

    pub fn topics(&self) -> &'static [&'static str] { self.course.and_then(|i| COURSES.get(i)).map(|c| c.1).unwrap_or(&[]) }

    /// The topic as written; None when the AI chooses (or "Other" is picked and nothing is typed).
    pub fn topic_name(&self) -> Option<String> {
        match self.topic {
            Topic::Ai => None,
            Topic::Listed(i) => self.topics().get(i).map(|t| t.to_string()),
            Topic::Typed => Some(self.topic_text.trim().to_string()).filter(|t| !t.is_empty()),
        }
    }

    /// A short name for a question the AI left unnamed.
    pub fn short_name(&self) -> String {
        match (self.topic_name(), self.course_name()) {
            (Some(t), course) => {
                let topic: String = t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect();
                course.map_or_else(|| topic.clone(), |course| format!("{topic} ({course})"))
            }
            (None, Some(c)) => format!("{c} question"),
            (None, None) => "Math question".into(),
        }
    }

    /// Keep the choices consistent after the course or the part count changes.
    pub fn fix(&mut self) {
        if self.course.is_some_and(|c| c >= COURSES.len()) { self.course = Some(0); }
        if let Topic::Listed(i) = self.topic {
            if i >= self.topics().len() { self.topic = Topic::Ai; }
        }
        if let Some(n) = self.part_count { self.part_count = Some(n.clamp(1, MAX_PARTS)); }
        if let Some([min, max]) = self.part_range.as_mut() {
            *min = (*min).clamp(1, MAX_PARTS);
            *max = (*max).clamp(*min, MAX_PARTS);
        }
        let want = self.part_range.map_or_else(|| self.part_count.unwrap_or(1), |[_, max]| max);
        let last = if self.part_range.is_some() { None } else { self.answers.last().copied().flatten() };
        self.answers.resize(want, last);
        if self.options.is_some_and(|n| !(2..=6).contains(&n)) { self.options = None; }
    }

    /// Rounding matters when a part may be a number (a number part, or one the AI picks).
    pub fn uses_rounding(&self) -> bool { self.answers.iter().any(|a| matches!(a, None | Some(Answer::Number))) }
    /// The option count matters when a part may be multiple choice.
    pub fn uses_options(&self) -> bool { self.answers.iter().any(|a| a.is_none_or(|a| a.has_options())) }

    /// What is still missing before the AI can write, in plain words; None when ready.
    pub fn missing(&self) -> Option<&'static str> {
        if self.topic == Topic::Typed && self.topic_text.trim().is_empty() { return Some("Type a topic first."); }
        None
    }

    pub(crate) fn checks_answer_setup(&self) -> bool {
        self.part_count.is_some() || self.part_range.is_some()
            || (!self.prefer_answer_types && self.answers.iter().any(Option::is_some))
    }

    pub(crate) fn answer_setup_issue(&self, types: &[&str]) -> Option<String> {
        let count = types.len();
        if let Some([min, max]) = self.part_range {
            if !(min..=max).contains(&count) {
                return Some(format!("Write {min}–{max} parts. This draft has {count}."));
            }
        } else if let Some(expected) = self.part_count
            && count != expected
        {
            return Some(format!("Write exactly {expected} part(s). This draft has {count}."));
        }
        if self.prefer_answer_types { return None; }
        for (index, actual) in types.iter().enumerate() {
            let slot = if self.part_count.is_none() && self.part_range.is_none() { 0 } else { index };
            if let Some(answer) = self.answers.get(slot).copied().flatten() {
                let (expected, _, _) = answer.details();
                if *actual != expected {
                    return Some(format!("Use MyOpenMath type {expected} for part {}. The draft uses {actual}.", index + 1));
                }
            }
        }
        None
    }

    /// The request the AI writer gets.
    pub fn request(&self) -> String {
        let course = self.course_name();
        let mut s = match (course, self.topic_name()) {
            (Some(c), Some(t)) => format!("{c}: write a MyOpenMath question on {t}."),
            (Some(c), None) => format!("{c}: write a MyOpenMath question on a topic you choose from this course."),
            (None, Some(t)) => format!("Write a MyOpenMath question on {t}, at the level of the course it usually belongs to."),
            (None, None) => "Write a MyOpenMath question on a high school or college math topic you choose.".into(),
        };
        s.push_str(&match self.difficulty {
            Some(d) => format!(" Difficulty: {}.", d.label().to_lowercase()),
            None => " Choose a difficulty that suits the topic.".into(),
        });
        if self.answers.iter().any(Option::is_some) {
            s.push_str(if self.prefer_answer_types {
                " Treat the selected answer types below as preferences. Use another type if it better fits the question."
            } else {
                " Use the selected IMathAS answer types for all parts that specify one."
            });
        }
        if let Some([min, max]) = self.part_range {
            s.push_str(&format!(" Write between {min} and {max} parts inclusive. Parts beyond {min} are optional. If writing more than one part, use IMathAS multipart with an ordered $anstypes entry for every included part."));
            for i in 0..max {
                let a = self.answers.get(i).copied().flatten();
                let optional = if i < min { "" } else { " if included" };
                s.push_str(&format!(" Part ({}){optional}: {}.", (b'a' + i as u8) as char, answer_spec(a, self)));
            }
        } else { match self.part_count {
            None => {
                s.push_str(" Choose how many parts it needs (one part is fine).");
                s.push_str(&match self.answers.first().copied().flatten() {
                    Some(a) => format!(" Every part: {}.", a.spec(self)),
                    None => format!(" For each part, {}.", answer_spec(None, self)),
                });
            }
            Some(1) => s.push_str(&format!(" Answer: {}.", answer_spec(self.answers.first().copied().flatten(), self))),
            Some(n) => {
                s.push_str(&format!(" It has {n} parts:"));
                for i in 0..n {
                    let a = self.answers.get(i).copied().flatten();
                    s.push_str(&format!(" part ({}) {}{}", (b'a' + i as u8) as char, answer_spec(a, self), if i + 1 == n { "." } else { ";" }));
                }
            }
        }
        }
        if self.uses_rounding() && self.rounding.is_none() && self.part_range.is_none() && self.part_count.is_none() && self.answers.first().copied().flatten().is_none() {
            s.push_str(" Number answers: if not whole, choose sensible rounding and say it in the question.");
        }
        s.push_str(match self.story {
            Some(true) => " Set it in a short, realistic real-world situation.",
            Some(false) => " Keep it purely mathematical, with no story.",
            None => " Use a real-world situation if it suits the topic.",
        });
        s.push_str(if self.random {
            " Use random numbers so every student gets different values, and make sure every set of numbers gives a sensible question."
        } else {
            " Use fixed numbers (every student sees the same question)."
        });
        s.push_str(match self.help_style {
            HelpStyle::DetailedSolution => WORKED_SOLUTION,
            HelpStyle::WrittenHelp => " Write a short conceptual hint in the Common Control variable $helptext. Explain how to get started without giving the answer or walking through this exact problem. Leave the Detailed Solution and ANSWER section empty. Do not invent links.",
        });
        let description = self.topic_text.trim();
        if self.topic != Topic::Typed && !description.is_empty() {
            s.push_str(&format!("\n\nQuestion description from the teacher: {description}"));
        }
        let extra = self.extra.trim();
        if !extra.is_empty() { s.push_str(&format!("\n\nThe teacher adds: {extra}")); }
        if let Some((title, spec)) = &self.figure {
            s.push_str(&format!("\n\nUse the configured {title} in the question. Recreate it with supported MyOpenMath drawing or graph macros. Keep the figure's parameters, labels, and the values read from it fixed across versions. Any randomization should only affect independent values. Make the question depend on the figure, and describe it for students who cannot see it. Figure settings: {spec}"));
        }
        s
    }
}

/// Every question gets a worked solution: it becomes MyOpenMath's Detailed Solution, which students
/// see after they answer.
const WORKED_SOLUTION: &str = " Always include a step-by-step worked solution for the student's own numbers in \
    $solutionguide (HTML using the question's variables, each step on its own line, ending with the answer); \
    it is shown as MyOpenMath's Detailed Solution after the student answers.";

/// The name MyOpenMath's question editor shows for a question type code in its Question type menu
/// (course/moddataset.php: `data-ln` where it has one, else the item's text).
pub fn type_menu_label(code: &str) -> &'static str {
    match code.trim() {
        "number" => "Number",
        "calculated" => "Calculated Number",
        "complex" => "Complex Number",
        "calccomplex" => "Calculated Complex Number",
        "choices" => "Multiple-choice",
        "multans" => "Multiple-answer",
        "matching" => "Matching",
        "numfunc" => "Algebraic Expression (Function)",
        "string" => "String",
        "essay" => "Essay",
        "file" => "File Upload",
        "draw" => "Drawing",
        "ntuple" => "N-Tuple",
        "calcntuple" => "Calculated N-Tuple",
        "matrix" => "Matrix",
        "calcmatrix" => "Calculated Matrix",
        "interval" => "Interval",
        "calcinterval" => "Calculated Interval",
        "complexntuple" => "Complex N-Tuple",
        "calccomplexntuple" => "Calculated Complex N-Tuple",
        "algntuple" => "Algebraic N-Tuple",
        "complexmatrix" => "Complex Matrix",
        "calccomplexmatrix" => "Calculated Complex Matrix",
        "algmatrix" => "Algebraic Matrix",
        "chemeqn" => "Chemical Equation",
        "molecule" => "Chemical Molecule",
        "multipart" => "Multipart",
        "conditional" => "Conditional",
        _ => "",
    }
}

/// Ways to change a question that already works.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Change { #[default] Easier, Harder, Numbers, Story, Wording, Solution, AddPart, Other }

impl Change {
    pub const ALL: [Change; 8] = [Change::Easier, Change::Harder, Change::Numbers, Change::Story, Change::Wording, Change::Solution, Change::AddPart, Change::Other];
    pub fn label(self) -> &'static str {
        match self {
            Change::Easier => "Make it easier",
            Change::Harder => "Make it harder",
            Change::Numbers => "Use different numbers",
            Change::Story => "Use a different story",
            Change::Wording => "Make the wording clearer",
            Change::Solution => "Make the worked solution clearer",
            Change::AddPart => "Add another part",
            Change::Other => "Something else (type it)",
        }
    }
    fn spec(self, other: &str) -> String {
        match self {
            Change::Easier => "Make it easier: simpler numbers and fewer steps.".into(),
            Change::Harder => "Make it harder: one more step of reasoning, or less friendly numbers.".into(),
            Change::Numbers => "Change the ranges of the random numbers (keep them sensible at every seed).".into(),
            Change::Story => "Use a different real-world situation; keep the math the same.".into(),
            Change::Wording => "Make the wording clearer for a student: say exactly what to enter and how to round.".into(),
            Change::Solution => "Rewrite the worked solution in $solutionguide: one short step per line, every number shown, ending with the answer.".into(),
            Change::AddPart => "Add one more part that builds on the existing parts.".into(),
            Change::Other => other.trim().to_string(),
        }
    }
}

pub(crate) fn with_writing_preference(request: &str, preference: &str) -> String {
    let request = request.trim();
    let preference = preference.trim();
    if preference.is_empty() {
        return request.to_string();
    }
    format!("{request}\n\nSaved teaching preference (follow when compatible with this question and the teacher's latest request):\n{preference}")
}

/// The request for changing an existing question: the original request, the current question file,
/// and the change.
pub fn change_request(original: &str, question_file: &str, change: Change, other: &str) -> String {
    format!("You wrote this MyOpenMath question for the request below. Revise it as asked and keep everything else the same.\n\n\
        REQUEST:\n{original}\n\nCURRENT QUESTION FILE:\n{question_file}\n\nCHANGE: {}", change.spec(other))
}

/// The writer's log line as a teacher should see it, or None for lines that only matter to a developer.
pub fn plain_progress(line: &str, max_tries: usize) -> Option<String> {
    let l = line.trim();
    if l.starts_with("writing") { return Some("Writing your question…".into()); }
    if l.starts_with("looked up") { return Some("Checking the MyOpenMath rules…".into()); }
    if l.contains("did not answer") { return Some("The AI was slow to answer. Trying again…".into()); }
    if l.starts_with("starting over") || l.starts_with("restart") { return Some("Starting over with a fresh attempt…".into()); }
    if let Some(rest) = l.strip_prefix("try ") {
        let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if l.contains("passes") || l.contains("passed") { return Some("It works with every set of numbers.".into()); }
        if l.contains("problem") || l.contains("caught") {
            return Some(format!("Found a problem. Fixing it (attempt {} of {max_tries})…", n.parse::<usize>().unwrap_or(1) + 1));
        }
        return Some("Testing it with 3 sets of random numbers…".into());
    }
    None
}

/// An error as a teacher should see it.
pub fn plain_error(e: &str) -> String {
    let l = e.to_ascii_lowercase();
    if l.contains("api key") || l.contains("401") || l.contains("unauthorized") {
        return "The AI isn't set up yet. Open Settings, then AI, and add your key.".into();
    }
    if l.contains("402") || l.contains("credit") {
        return "Your AI account is out of credit. Add credit with your AI provider, or pick another AI in Settings.".into();
    }
    if l.contains("can't reach") || l.contains("timeout") || l.contains("timed out") || l.contains("connection") {
        return "Couldn't reach the AI. Check your internet connection and try again.".into();
    }
    if l.contains("429") || l.contains("answered 5") || l.contains("503") || l.contains("502") || l.contains("overloaded") || l.contains("rate limit") {
        return "The AI is busy right now. Wait a minute and try again.".into();
    }
    if l.contains("engine") {
        return if cfg!(target_arch = "wasm32") {
            "The question checker didn't start. Reload the page and try again.".into()
        } else {
            "The question checker didn't start. Try again; if it keeps failing, close D.A.D and open it again.".into()
        };
    }
    // A sentence already written for the teacher (the preview, the checker) is shown as it is;
    // only raw errors get the general line, which does not blame the description.
    let written_for_teachers = e.len() < 220 && e.starts_with(|c: char| c.is_ascii_uppercase())
        && !["{", "error:", "Error:", "::", "os error", "JSON", "Failed to load", "404", "500", "No such file", "panicked", "Unexpected", "http", "answered"].iter().any(|raw| e.contains(raw));
    if written_for_teachers { return e.to_string(); }
    "That didn't work. Try again; if it keeps happening, change the description or pick another AI.".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_course_but_other_has_topics() {
        assert_eq!(COURSES.last().unwrap().0, "Other");
        assert!(COURSES[..COURSES.len() - 1].iter().all(|(_, t)| !t.is_empty()));
    }

    #[test]
    fn the_default_leaves_the_choices_to_the_ai() {
        let r = Form::default().request();
        // No course either: a teacher who never opens the Course menu is not given Integrated Math 1.
        assert!(r.starts_with("Write a MyOpenMath question on a high school or college math topic you choose."), "{r}");
        assert!(r.contains("Choose a difficulty") && r.contains("Choose how many parts") && r.contains("an answer type you choose"), "{r}");
        assert!(r.contains("real-world situation if it suits") && r.contains("random numbers") && r.contains("worked solution") && r.contains("$solutionguide"), "{r}");
    }

    #[test]
    fn picked_choices_name_the_type_and_rounding() {
        let f = Form { course: Some(7), topic: Topic::Listed(5), part_count: Some(2), answers: vec![Some(Answer::Number), Some(Answer::Choice)],
            difficulty: Some(Difficulty::Hard), rounding: Some(Rounding::Two), options: Some(3), story: Some(false), ..Form::default() };
        let r = f.request();
        assert!(r.contains("Statistics: write a MyOpenMath question on Normal distribution. Difficulty: hard."), "{r}");
        assert!(r.contains("It has 2 parts: part (a) the student types a number (rounded to 2 decimal places") && r.contains("; part (b) multiple choice with 3 options"), "{r}");
        assert!(r.contains("no story") && r.contains("worked solution"), "{r}");
    }

    #[test]
    fn an_ai_part_inside_picked_parts() {
        let f = Form { part_count: Some(2), answers: vec![Some(Answer::Expression), None], ..Form::default() };
        let r = f.request();
        assert!(r.contains("part (a) the student types an algebraic expression") && r.contains("part (b) an answer type you choose"), "{r}");
        assert!(f.uses_rounding() && f.uses_options());
    }

    #[test]
    fn picked_type_can_be_required_or_preferred() {
        let mut form = Form {
            part_count: Some(1),
            answers: vec![Some(Answer::Expression)],
            ..Form::default()
        };
        assert!(form.request().contains("Use the selected IMathAS answer types"));
        assert!(form.request().contains("IMathAS type numfunc"));
        form.prefer_answer_types = true;
        let request = form.request();
        assert!(request.contains("as preferences"));
        assert!(request.contains("IMathAS type numfunc"));
    }

    #[test]
    fn part_range_supports_mixed_types_and_optional_parts() {
        let mut form = Form {
            part_range: Some([2, 3]),
            answers: vec![Some(Answer::Text), Some(Answer::Drawing), Some(Answer::SelectAll)],
            ..Form::default()
        };
        form.fix();
        let request = form.request();
        assert!(request.contains("between 2 and 3 parts"));
        assert!(request.contains("Part (c) if included"));
        assert!(form.answer_setup_issue(&["string", "draw"]).is_none());
        assert!(form.answer_setup_issue(&["string", "draw", "multans"]).is_none());
        assert!(form.answer_setup_issue(&["string"]).is_some());
        assert!(form.answer_setup_issue(&["string", "number"]).is_some());
        form.prefer_answer_types = true;
        assert!(form.answer_setup_issue(&["string", "number"]).is_none());
        assert!(form.answer_setup_issue(&["string"]).is_some());
    }

    #[test]
    fn no_course_and_no_topic_still_asks_for_something() {
        let f = Form { course: None, ..Form::default() };
        assert!(f.request().starts_with("Write a MyOpenMath question on a high school or college math topic you choose."));
        assert_eq!(f.short_name(), "Math question");
        let f = Form { course: None, topic: Topic::Typed, topic_text: "Venn diagrams".into(), ..Form::default() };
        assert!(f.request().starts_with("Write a MyOpenMath question on Venn diagrams"));
    }

    #[test]
    fn fix_keeps_one_answer_per_part() {
        let mut f = Form { part_count: Some(3), answers: vec![Some(Answer::Choice)], ..Form::default() };
        f.fix();
        assert_eq!(f.answers, vec![Some(Answer::Choice); 3]);
        f.part_count = None;
        f.fix();
        assert_eq!(f.answers.len(), 1);
        let mut f = Form { course: Some(1), topic: Topic::Listed(40), ..Form::default() };
        f.fix();
        assert_eq!(f.topic, Topic::Ai);
    }

    #[test]
    fn a_typed_topic_must_be_typed() {
        let mut f = Form { topic: Topic::Typed, ..Form::default() };
        assert_eq!(f.missing(), Some("Type a topic first."));
        f.topic_text = "Venn diagrams".into();
        assert!(f.missing().is_none() && f.request().contains("on Venn diagrams"));
    }

    #[test]
    fn a_change_carries_the_question() {
        let r = change_request("orig", "// === QUESTION TEXT ===", Change::Easier, "");
        assert!(r.contains("REQUEST:\norig") && r.contains("CURRENT QUESTION FILE") && r.contains("CHANGE: Make it easier"));
        assert!(change_request("o", "f", Change::Other, " use pizza ").ends_with("CHANGE: use pizza"));
    }

    #[test]
    fn type_codes_have_menu_names() {
        assert_eq!(type_menu_label("number"), "Number");
        assert_eq!(type_menu_label(" multipart\n"), "Multipart");
        assert_eq!(type_menu_label("nonsense"), "");
    }

    #[test]
    fn progress_and_errors_are_plain() {
        assert_eq!(plain_progress("try 1: \"x\" has 2 problem(s)", 4).as_deref(), Some("Found a problem. Fixing it (attempt 2 of 4)…"));
        assert_eq!(plain_progress("try 2: \"x\" passes at seeds 4242, 7, 99", 4).as_deref(), Some("It works with every set of numbers."));
        assert_eq!(plain_progress("    Caught error while evaluating", 4), None);
        assert!(plain_error("The AI did not answer: OpenRouter needs an API key").contains("Settings"));
    }
}
