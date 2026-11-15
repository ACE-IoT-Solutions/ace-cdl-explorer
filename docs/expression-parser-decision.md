# Equation parser decision (WP4)

Keep `meval` 0.2 behind the optional, default-off `equations` feature for this move.
The Explorer and full verification adapter opt in. Core clients that do not enable
that feature acquire no expression-parser dependency. A graph containing free-form
equations fails explicitly without it; documentary equations on recognized builtin
families retain their pre-migration precedence.

This is a compatibility decision, not a claim that meval is actively maintained or
that another parser would be slower. The existing formula evaluator is f64-based,
uses meval's context/functions and evaluates ordered output assignments. Both frozen
original engines and every existing equation regression remain independent oracles.
Changing the parser would be a behavior change requiring its own compatibility work.

| Candidate reviewed | Relevant API | Compatibility assessment |
| --- | --- | --- |
| [meval 0.2](https://docs.rs/meval/0.2.0/meval/) | f64 expressions, variable/function context, custom functions and compiled/bound expressions | Preserves the existing grammar, function naming and numeric behavior without an adapter. |
| [fasteval 0.2.4](https://docs.rs/fasteval/0.2.4/fasteval/) | Parsed/compiled algebraic expressions, reusable slab, variables and custom functions | Plausible future candidate. Builtin constants use function syntax such as `pi()`/`e()`, differing from meval's variable constants; grammar and builtin behavior need explicit compatibility tests. |
| [evalexpr 13.1](https://docs.rs/evalexpr/13.1.0/evalexpr/) | Reusable expressions, typed numeric/Boolean values, mutable contexts and assignment syntax | Offers more language features than this f64 boundary needs. Its distinct integer/float values and type-safe mutable contexts require a deliberate adapter and numeric-equivalence tests. |

The review compares documented APIs, not measured performance. No replacement is
introduced. Before adopting one, test the saved equation corpus plus precedence,
function arity, constants, invalid/nonfinite results, assignment order and repeated
stepping against both original engines, then benchmark native and wasm. Keep that
work separate from the architecture migration and its bit-exact annual acceptance.
