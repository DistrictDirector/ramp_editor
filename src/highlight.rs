use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter, Theme, ThemeSet};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};
use quartz::Color;

pub type Spans = Vec<(Color, String)>;

/// Merge adjacent spans that share the same color to reduce token object count.
pub fn merge_adjacent(spans: &[(Color, String)]) -> Vec<(Color, String)> {
    let mut out: Vec<(Color, String)> = Vec::new();
    for (color, text) in spans {
        let same = out.last()
            .map(|(c, _)| c.0 == color.0 && c.1 == color.1 && c.2 == color.2)
            .unwrap_or(false);
        if same {
            out.last_mut().unwrap().1.push_str(text);
        } else {
            out.push((*color, text.clone()));
        }
    }
    out
}

/// Parser + highlighter state captured at the start of each document line.
/// Storing ScopeStack (rather than HighlightState directly) keeps this Clone
/// without relying on HighlightState's internal layout across syntect versions.
#[derive(Clone)]
struct LineState {
    parse:       ParseState,
    scope_stack: ScopeStack,
}

/// Per-document-row syntax highlight cache with incremental reprocessing.
///
/// The highlight work is fully decoupled from the slot/scroll system:
///   mark_dirty_from(row)  — called on every edit, O(1)
///   rehighlight(lines)    — called once per on_update tick when dirty,
///                           reprocesses only lines >= dirty row
///   line_spans(row)       — called during slot rebuild, O(1) lookup
///
/// Scroll and cursor moves never touch this struct.
pub struct SyntaxHighlighter {
    ss:          SyntaxSet,
    theme:       Theme,
    /// line_states[i] is the parser state *before* line i.
    /// Length is always lines.len() + 1 after a rehighlight.
    line_states: Vec<LineState>,
    /// Cached highlighted spans for each document row.
    cache:       Vec<Spans>,
    /// First dirty document row. usize::MAX means fully clean.
    pub dirty:   usize,
}

impl std::fmt::Debug for SyntaxHighlighter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SyntaxHighlighter(dirty={})", self.dirty)
    }
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let ss    = SyntaxSet::load_defaults_newlines();
        let ts    = ThemeSet::load_defaults();
        let theme = ts.themes["base16-ocean.dark"].clone();
        Self {
            ss,
            theme,
            line_states: Vec::new(),
            cache:       Vec::new(),
            dirty:       0,   // triggers a full highlight on first tick
        }
    }

    /// Mark a single row dirty. Takes the min of the existing dirty row so
    /// that earlier edits always win — we never skip re-highlighting rows that
    /// precede a later edit in the same tick.
    pub fn mark_dirty_from(&mut self, row: usize) {
        self.dirty = self.dirty.min(row);
    }

    /// Reprocess all lines from self.dirty onward using saved parser states.
    /// Lines before self.dirty are untouched — their cached spans remain valid.
    /// After this call self.dirty == usize::MAX (fully clean).
    pub fn rehighlight(&mut self, lines: &[String]) {
        let n = lines.len();

        if n == 0 {
            self.cache.clear();
            self.line_states.clear();
            self.dirty = usize::MAX;
            return;
        }

        if self.dirty >= n {
            self.dirty = usize::MAX;
            return;
        }

        let syntax      = self.ss.find_syntax_by_extension("rs")
                              .unwrap_or_else(|| self.ss.find_syntax_plain_text());
        let highlighter = Highlighter::new(&self.theme);

        // Grow / shrink storage to match current line count.
        self.cache.resize_with(n, Vec::new);

        let needed = n + 1;
        if self.line_states.len() < needed {
            let placeholder = self.line_states.last().cloned().unwrap_or(LineState {
                parse:       ParseState::new(syntax),
                scope_stack: ScopeStack::new(),
            });
            self.line_states.resize_with(needed, || placeholder.clone());
        }
        self.line_states.truncate(needed);

        // When dirty == 0 we must reset the initial parser state so that we
        // don't carry over stale parse state from a previous document load.
        if self.dirty == 0 {
            self.line_states[0] = LineState {
                parse:       ParseState::new(syntax),
                scope_stack: ScopeStack::new(),
            };
        }

        // Incremental reprocess from the first dirty row.
        for row in self.dirty..n {
            // Clone state before this line. parse_line mutates parse in place;
            // we reconstruct HighlightState from the scope_stack snapshot.
            let LineState { mut parse, scope_stack } = self.line_states[row].clone();
            let mut hl = HighlightState::new(&highlighter, scope_stack);

            // syntect requires a trailing newline for correct tokenisation.
            let line = format!("{}\n", lines[row]);
            let ops  = parse.parse_line(&line, &self.ss).unwrap_or_default();

            let spans: Spans = {
                let iter = HighlightIterator::new(&mut hl, &ops, &line, &highlighter);
                iter.map(|(style, text)| {
                    let c = style.foreground;
                    (Color(c.r, c.g, c.b, 255), text.trim_end_matches('\n').to_string())
                })
                .filter(|(_, t)| !t.is_empty())
                .collect()
            };

            self.cache[row] = spans;

            // Persist the parser state after this line so the next line can
            // pick up exactly where we left off.
            if row + 1 < self.line_states.len() {
                self.line_states[row + 1] = LineState {
                    parse,
                    scope_stack: hl.path.clone(),
                };
            }
        }

        self.dirty = usize::MAX;
    }

    /// Returns the cached highlighted spans for a document row, or None if the
    /// cache hasn't been built yet (e.g. the very first tick before rehighlight).
    pub fn line_spans(&self, row: usize) -> Option<&Spans> {
        self.cache.get(row)
    }
}