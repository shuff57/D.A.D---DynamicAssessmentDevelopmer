//! A parameterized MyOpenMath question built from a verSHion coordinate grid.

use super::MomQuestion;
use crate::figures::{Params, math::integer_line};
use serde::{Deserialize, Serialize};

const MAX_SLOPE: u32 = 20;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct LineVariant {
    pub enabled: bool,
    pub slope: [i32; 2],
    pub intercept: [i32; 2],
    pub read_x: [i32; 2],
    pub mark_point: bool,
    x_axis: [i32; 2],
    y_axis: [i32; 2],
    color: String,
    #[serde(default)]
    square_cells: bool,
}

impl LineVariant {
    pub fn from_grid(grid: &Params) -> Option<Self> {
        if grid.s("xunits") != "Numbers"
            || grid.f("xstep") != 1.0 || grid.f("ystep") != 1.0
            || !grid.b("numbers") || grid.s("xnum") != "Every line" || grid.s("ynum") != "Every line"
            || !grid.s("points").trim().is_empty()
            || !matches!(grid.s("xlabel"), "" | "x") || !matches!(grid.s("ylabel"), "" | "y")
            || grid.s("xtitle_mode") != "None" || grid.s("ytitle_mode") != "None"
            || grid.s("title_mode") != "None" && !grid.s("title").trim().is_empty()
            || grid.b("eq_labels")
        {
            return None;
        }
        let axis = |lo, hi| {
            let (lo, hi) = (grid.f(lo), grid.f(hi));
            if lo.fract() != 0.0 || hi.fract() != 0.0 || lo < -1000.0 || hi > 1000.0 {
                return None;
            }
            let bounds = [lo as i32, hi as i32];
            supported_axis(bounds).then_some(bounds)
        };
        let x_axis = axis("xmin", "xmax")?;
        let y_axis = axis("ymin", "ymax")?;
        let (slope, intercept) = integer_line(grid.s("equations"))?;
        let color = grid.s("eq1_color");
        if !supported_color(color) {
            return None;
        }
        let read_x = (x_axis[0]..=x_axis[1])
            .filter(|&x| contains(y_axis, line_y(slope, intercept, x)))
            .min_by_key(|&x| x.abs_diff(2))?;
        Some(Self {
            enabled: false,
            slope: [slope; 2],
            intercept: [intercept; 2],
            read_x: [read_x; 2],
            mark_point: false,
            x_axis,
            y_axis,
            color: color.into(),
            square_cells: grid.b("square"),
        })
    }

    pub fn question(&self) -> Result<MomQuestion, &'static str> {
        if !supported_axis(self.x_axis) || !supported_axis(self.y_axis) || !supported_color(&self.color) {
            return Err("Reopen the figure and choose a numbered grid with both axes visible.");
        }
        if [self.slope, self.intercept, self.read_x].iter().any(|[lo, hi]| lo > hi) {
            return Err("Set each starting value at or below its ending value.");
        }
        if self.slope[0] == self.slope[1] && self.intercept[0] == self.intercept[1] {
            return Err("Give the slope or y-intercept a range so the line can change.");
        }
        if self.slope.iter().any(|slope| slope.unsigned_abs() > MAX_SLOPE) {
            return Err("Keep the slope between −20 and 20 so the line is readable on this grid.");
        }
        if self.read_x.iter().any(|&x| !contains(self.x_axis, x.into())) {
            return Err("Keep the x-values students will read inside the figure’s horizontal axis.");
        }
        // A linear expression reaches its extrema over independent ranges at these corners.
        for slope in self.slope {
            for intercept in self.intercept {
                for x in self.read_x {
                    if !contains(self.y_axis, line_y(slope, intercept, x)) {
                        return Err("Some versions place the answer outside the vertical axis. Narrow the ranges or widen the grid in Figures.");
                    }
                }
            }
        }

        let choose = |[lo, hi]: [i32; 2]| if lo == hi { format!("{lo}") } else { format!("rand({lo},{hi})") };
        let (slope, intercept, read_x) = (choose(self.slope), choose(self.intercept), choose(self.read_x));
        // Give tick numerals room at the ends of the axes. The window is identical at every seed.
        let [x_lo, x_hi] = self.x_axis;
        let [y_lo, y_hi] = self.y_axis;
        let [xmin, xmax] = [x_lo - 1, x_hi + 1];
        let [ymin, ymax] = [y_lo - 1, y_hi + 1];
        let (x_span, y_span) = (xmax - xmin, ymax - ymin);
        let (width, height) = if self.square_cells {
            let span = x_span.max(y_span);
            (420 * x_span / span, 420 * y_span / span)
        } else {
            (420, 420)
        };
        let color = match self.color.as_str() {
            "#e9f1fb" | "#c9def4" => "#1f5fbf",
            color => color,
        };
        let plot = if self.mark_point {
            r#"array($dadline, "dot,$dadreadx,$answer,closed,black")"#
        } else {
            "$dadline"
        };
        let control = format!(
            r#"$dadslope = {slope}
$dadintercept = {intercept}
$dadreadx = {read_x}
$answer = $dadslope*$dadreadx + $dadintercept
$dadline = "($dadslope)*x + ($dadintercept),{color},,,,,2"
$dadgraph = showplot({plot},{xmin},{xmax},{ymin},{ymax},"1:1:x:y",1,{width},{height})
$dadcoords = "Line graph. Points on the numbered grid: "
for ($dadx={x_lo}..{x_hi}) {{
  $dady = $dadslope*$dadx + $dadintercept
  $dadcoords = $dadcoords . "At x = $dadx, y = $dady. " if ($dady >= {y_lo} && $dady <= {y_hi})
}}
$dadgraph = replacealttext($dadgraph, $dadcoords)
$solutionguide = "<p>Find x = $dadreadx on the horizontal axis and move vertically to the line.</p><p>Read across to the vertical axis. The y-value is $answer.</p>""#
        );
        Ok(MomQuestion {
            name: "Read a y-value from a line graph".into(),
            qtype: "number".into(),
            control,
            qtext: r#"<p>Use the graph to find the value of y when x = $dadreadx.</p>
<div class="dad-graph"><div>$dadgraph</div><span class="dad-graph-description" hidden aria-hidden="true">$dadcoords</span></div>
<p>y = $answerbox</p>"#.into(),
            answer_code: "$solutionguide".into(),
            ..Default::default()
        })
    }
}

fn supported_axis([lo, hi]: [i32; 2]) -> bool {
    lo <= 0 && hi >= 0 && (2..=20).contains(&(i64::from(hi) - i64::from(lo)))
}

fn supported_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn contains([lo, hi]: [i32; 2], value: i64) -> bool {
    (i64::from(lo)..=i64::from(hi)).contains(&value)
}

fn line_y(slope: i32, intercept: i32, x: i32) -> i64 {
    i64::from(slope) * i64::from(x) + i64::from(intercept)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Params {
        let def = crate::figures::registry().into_iter().find(|def| def.id == "coordinate-grid").unwrap();
        Params::defaults(&(def.params)())
    }

    #[test]
    fn reads_the_selected_line_and_rejects_ineligible_figures() {
        let mut params = grid();
        let original = LineVariant::from_grid(&params).unwrap();
        assert_eq!(original.slope, [2, 2]);
        assert_eq!(original.intercept, [1, 1]);
        assert_eq!(original.read_x, [2, 2]);
        assert!(!original.enabled);

        for equations in ["y = x^2", "y = x^1e0", "y = x/2", "y = x; y = 2x", "x = 2", "y = x, x > 0"] {
            params.0.insert("equations".into(), crate::figures::Value::S(equations.into()));
            assert!(LineVariant::from_grid(&params).is_none(), "{equations}");
        }
        let mut titled = grid();
        titled.0.insert("xtitle_mode".into(), crate::figures::Value::S("Text".into()));
        assert!(LineVariant::from_grid(&titled).is_none());
    }

    #[test]
    fn graph_and_answer_share_parameters_and_question_round_trips() {
        let mut variant = LineVariant::from_grid(&grid()).unwrap();
        variant.slope = [-2, -1];
        variant.intercept = [6, 9];
        variant.read_x = [1, 3];
        variant.mark_point = true;
        let question = variant.question().unwrap();
        assert!(question.control.contains("$dadslope = rand(-2,-1)"));
        assert!(question.control.contains("$answer = $dadslope*$dadreadx + $dadintercept"));
        assert!(question.control.contains("($dadslope)*x + ($dadintercept),"));
        assert!(question.control.contains("dot,$dadreadx,$answer,closed,black"));
        assert!(question.control.contains("replacealttext($dadgraph"));
        assert_eq!(MomQuestion::parse(&question.emit()), question);
    }

    #[test]
    fn prevents_fixed_lines_reversed_ranges_and_off_grid_answers() {
        let mut variant = LineVariant::from_grid(&grid()).unwrap();
        assert!(variant.question().unwrap_err().contains("so the line can change"));
        variant.slope = [4, 1];
        assert!(variant.question().unwrap_err().contains("starting value"));
        variant.slope = [1, 4];
        variant.read_x = [2, 16];
        assert!(variant.question().unwrap_err().contains("horizontal axis"));
        variant.read_x = [2, 5];
        assert!(variant.question().unwrap_err().contains("vertical axis"));
        variant.read_x = [2, 3];
        assert!(variant.question().is_ok());
        variant.slope = [100_000_000; 2];
        variant.intercept = [-200_000_000, -199_999_999];
        variant.read_x = [2; 2];
        assert!(variant.question().unwrap_err().contains("readable"));
    }
}
