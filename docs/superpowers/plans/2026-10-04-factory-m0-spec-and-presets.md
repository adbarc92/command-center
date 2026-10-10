# Factory M0: spec and presets — Implementation Plan

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking. This plan has two lanes. Each lane is executed by one agent, alone, in its own worktree; the two lanes share no file and may run at the same time. Follow a lane task by task with superpowers:executing-plans. Every code block that follows a `plan-op` comment is the exact text to write; nothing is abbreviated.

**Goal:** Ship two pure Rust contract crates in `command-center`. `factory-spec` holds the format of a spec file, its parsing, its validation, its split into child units, and the scope and overlap rules. `factory-presets` holds the `cargo` and `node` stack rules: reading a JUnit report into test ids and outcomes, enumerating test ids from source, the criterion marker in a test name, protected path patterns, byte ranges of in-file test code, reading a dependency manifest, and the fixed secret-detection rules. Each crate ships a `testkit` and shared test vectors.

**Architecture:** Both crates are libraries with no I/O: bytes or text in, data out. The control plane and the harness link the same code, so the two decide spec readiness, scope overlap and test outcomes identically and cannot drift apart. `factory-spec` reads the YAML header with serde and the Markdown body with a small line-oriented reader; `factory-presets` puts two static implementations of one `Preset` trait behind `preset(name)`, over a streaming XML reader and two hand-written source scanners. Each crate compiles its vector files into itself, so a second repository runs the same data through the same code.

**Tech Stack:** Rust 2021 (toolchain 1.93), `serde` 1, `serde-saphyr` 1.3 (YAML), `sha2` 0.10, `quick-xml` 0.42, `toml` 1, `serde_json` 1; `regex` 1 as a dev-dependency only.

**Spec:** The ReqDrive factory design v0.4, in the private nexus repository: https://github.com/adbarc92/nexus/blob/main/docs/specs/2026-10-04-reqdrive-factory-design.md

## Global Constraints

**Rules for both lanes.**

- **Purity.** No file, network, process, environment, clock or thread access, and no `async`, anywhere under `src/`. `include_str!` is allowed: it reads at compile time. Each lane's last task greps for this.
- **Ownership.** A lane writes only under the directory it owns (`crates/factory-spec/**` or `crates/factory-presets/**`). It never edits the root `Cargo.toml`, `Cargo.lock`, a CI workflow, `docs/STATUS.md`, `CLAUDE.md` or a roadmap file. If a build changes `Cargo.lock`, the skeleton's dependencies differ from this plan: stop and report to the coordinator, do not commit the lockfile.
- **Worktree.** All work happens in the worktree the lane names. The main checkout of every repository is off limits: no edits, commits, stashes or branch switches there, because it may hold someone's unsaved work.
- **Test first** (doctrine C1). For each behaviour: write its tests, run them, see them fail, and only then write the code. The "expected" line under each run step was recorded by replaying this plan in a fresh workspace; if the real output differs in kind (a pass where a failure is expected, or the reverse), stop and find out why.
- **Frozen names.** The public names in this plan come from the program's shared-interface draft. Do not rename or reshape them. If a task cannot be done without changing one, stop and report.
- **Nothing else.** Do what the tasks say. Anything else you notice goes in the final report, not in the diff.
- **No cost, no outside contact.** This work does not deploy, publish, call a paid API or post anywhere.
- **Public repository.** `command-center` is public. Nothing written in the private nexus repository may be copied here, whether into code, a comment, a commit message or the pull request. Doctrine is cited by principle id only.
- **Commits and the pull request.** No `Co-Authored-By` line and no "Generated with" footer, anywhere. Each lane opens one pull request against `factory/m0` and leaves it open: it never merges, never pushes to `main` and never rewrites pushed history.

**Conventions, taken from `crates/fleet-core` and `crates/harness-protocol`.**

- Tests live in the file they test, in a `#[cfg(test)] mod tests` block at the bottom. Test names are sentences in snake case.
- Every module opens with a `//!` comment saying what it is for.
- Data types derive `Debug, Clone, PartialEq, Eq` and, where they cross a wire, `Serialize, Deserialize`. Enums serialise `snake_case`; the tier serialises lower case (`t1`), as `fleet-core`'s does.
- Errors are plain enums with a hand-written `Display`, an `impl std::error::Error`, and a `code()` returning a stable short name. These two crates do not use `thiserror`: their dependency lists are fixed by the program plan.
- Source files are ASCII and use LF line endings. Non-ASCII test data is written with `\u{...}` escapes.
- `cargo clippy ... -D warnings` and `cargo fmt --check` must pass at the end of the lane. Between tasks, a warning about a helper that the next task starts using is expected and is not a reason to stop.

**Decisions this plan builds on.** They are settled; do not reopen them.

1. *The spec file.* A spec is a Markdown file that starts with a YAML header between two `---` lines. The header has thirteen fields: `id`, `work_item`, `kind` (`feature`, `bugfix`, `chore`), `tier` (`t1`, `t2`, `t3`), the four scope lists `touched_files`, `create_paths`, `test_paths`, `touched_tests`, then `permitted_dependencies`, `expected_red`, `forms_affected`, `source_hash` and `template_version`. The body has sections for the intent, a reproduction (bug fixes), the invariants with the unconfirmed ones listed apart, the acceptance criteria (each with an id and an oracle), the open questions, and the split.
2. *The signature lives outside the file.* A signature is a SHA-256 taken over the file exactly as stored. So the parser must never tidy the bytes: a CRLF or a byte-order mark is inside what was signed.
3. *Readiness.* Six things stop a spec from being signed: anything still listed as an open question or as an unconfirmed invariant; a scope with nothing in it, or no `test_paths`; an acceptance criterion that lacks its id or its oracle; an id used by two criteria; a scope entry that touches some Form's interface while the header's `forms_affected` leaves that Form out; and a split that fails its own checks. Which paths are a Form's interface is not this crate's knowledge: the caller passes that map in.
4. *Scope.* An entry in a scope list is one of two things: the path of a single file, from the repository root, or a directory written as a prefix with a trailing `/`. Wildcards do not exist. What a unit may touch is all four of its lists together, plus whatever paths it has been granted since. Two units collide when a path of one is the same as, lies inside, or encloses a path of the other; the comparison ignores letter case and treats `\` and `/` alike. Removing a file is touching it, and renaming one is touching both the old and the new path.
5. *Split.* A split passes four mechanical checks: the children share out the parent's criteria with none left over and none held twice; no child reaches outside the parent's scope; two children in one layer have disjoint scopes; and every dependency points at a child in a lower-numbered layer. A "split" of one child, or of none, is not a split.
6. *Children have no files.* A child unit is identified by the parent's bytes plus a child id; `child_view` looks the child's criteria and scope up in the parent.
7. *Test ids.* The id of a test is built from the two things its report gives, a class name and a test name, combined into one string in a fixed way; no two tests may share one. This plan fixes the joiner as `" > "`.
8. *Criterion markers.* A test says which criterion it checks by carrying a short token in its name. The token stands alone: `ac1` marks `AC-1`, and the `ac1` inside `ac10` marks nothing.
9. *Protected patterns.* For `cargo`: `build.rs`, `.cargo/config.toml`, `rust-toolchain`, `rust-toolchain.toml`. For `node`: vitest, jest and mocha configuration files and `.npmrc`. For both, anywhere in the tree: everything under a `.claude/` directory, `.mcp.json`, `AGENTS.md`, `CLAUDE.md`.
10. *Manifests.* The only change a manifest may undergo is new dependency entries, in `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` and `[workspace.dependencies]` of a `Cargo.toml`, or in `dependencies` and `devDependencies` of a `package.json`. Any other difference is an error: a script, a `[[test]]` target, a feature, a `build =` line, anything.
11. *Enumeration is all or nothing.* If a file contains even one test whose name the preset cannot read as fixed text (a name built at run time, for instance), `enumerate_ids` fails for the whole file. It never returns the tests it could read and stays quiet about the rest, because the caller would then freeze an incomplete list.

**Two contracts that cross the lanes.** Both lanes rely on these and neither may change them alone.

- A criterion id is `AC-` followed by a decimal number with no leading zero (`AC-1`, `AC-27`). `factory-spec` treats any other heading as a criterion with no id. The marker for `AC-<n>` is `ac<n>`, lower case; `factory-presets` returns `AC-<n>` from `criterion_of`.
- Paths compare case-insensitively with `\` read as `/` in both crates.

**Where this plan goes beyond, or narrows, the program plan's shared-interface section.** Each is recorded here so the reviewer can accept or reject it; the lane's final report repeats them.

| # | Crate | What | Why |
|---|---|---|---|
| D1 | `factory-spec` | `serde_json` as a **dev**-dependency | To test that a `Problem` serialises with its `code()` as the tag. The library itself still links only `serde`, the YAML reader and `sha2` |
| D2 | `factory-spec` | Extra public types `Kind`, `Tier`, `PathFault`, and the constant `TEMPLATE_VERSION` | `Spec` and `Problem` need them |
| D3 | `factory-spec` | Refusals beyond decision 3: a malformed scope entry (`PathMalformed`), no criteria at all (`NoCriteria`), a child with no `test_paths`, two children with one id, a child naming an unknown criterion or dependency | Each closes a hole the listed refusals leave: a `..` segment makes "inside the parent" a text trick; a spec with no criteria passes every per-criterion check vacuously; a child is a full unit and needs somewhere to put its tests |
| D4 | `factory-spec` | A split that lists exactly **one** child is a `Problem` (`SplitTooSmall`); no children at all is not | A one-child split is ambiguous: the author asked for a split and did not get one. Refusing costs a one-line edit; silently running the whole parent scope as one unit could surprise. Open question Q1 |
| D5 | `factory-spec` | `Scope::with_grants` appends grants to `touched_files` | `Scope` has exactly four lists. A grant permits a change and carries no test meaning |
| D6 | `factory-spec` | Overlap also holds between a file `a/b` and any entry under `a/b/` | A tree cannot hold both; treating them as disjoint would admit two units that must conflict |
| D7 | `factory-spec` | An unknown `## ` section, a repeated section, an unclosed code fence and an unknown header field are parse errors | Each could otherwise hide open questions or silently empty a scope list |
| D8 | `factory-presets` | `Preset: Send + Sync` | `preset()` hands out `&'static dyn Preset`; without the bound it cannot be held across an `await` or shared between threads |
| D9 | `factory-presets` | `serde` as a dependency, beside the XML, TOML and JSON readers | `TestCase` and `TestStatus` travel in evidence; the vector index is read with it |
| D10 | `factory-presets` | Extra public items: `ID_SEPARATOR`, `test_id`, `MAX_REPORT_BYTES`, `MAX_REPORT_DEPTH`, `Side`, `CargoPreset`, `NodePreset` | Callers need the joiner and the limits; the two structs let a test build a preset without the lookup |
| D11 | `factory-presets` | `criterion_of` returns the criterion id (`AC-1`), not the raw marker (`ac1`) | The caller compares against criterion ids, and the two crates do not link each other |
| D12 | `factory-presets` | Protected patterns add `.cargo/config`, `.config/nextest.toml`, `vite.config.*` and `vitest.workspace.*` | Each configures the test runner or the build as surely as the listed files do |
| D13 | `factory-presets` | An added dependency that does not come from the default registry by name (path, git URL, other registry, alias, tarball) is an error (`UnsupportedSource`) | The caller checks added *names* against `permitted_dependencies`; a name check cannot vouch for a foreign source |
| D14 | `factory-presets` | A test that was retried (`flakyFailure`, `rerunFailure` and their `Error` forms) is not `Passed` | An id must be present once with no failure. Open question Q7 |
| D15 | `factory-presets` | No function for "run setup without lifecycle scripts" | The shared-interface section has none, and it depends on spike S7. Not built here |

## Requests to the coordinator

Lane CC-COORD owns the root `Cargo.toml`, `Cargo.lock`, CI and each crate's skeleton. This plan assumes the following is already merged into `factory/m0`. Both lanes check it in their first step and stop if it is not so.

**What is new relative to the scaffold plan as drafted on 2026-10-04.** That plan's skeleton declares `serde` and `sha2` for `factory-spec`, and `serde` and `serde_json` for `factory-presets`. This plan needs five more lines, all of which change `Cargo.lock`, so they are the coordinator's to add:

- `factory-spec`: `serde-saphyr` under `[dependencies]`; `serde_json` under `[dev-dependencies]`.
- `factory-presets`: `quick-xml` and `toml` under `[dependencies]`; `regex` under `[dev-dependencies]`.

None of the five does I/O, spawns a process or pulls in an async runtime, so the dependency-direction check should accept them; if that check works from an allow-list, these names need adding to it.

**1. Both crates are workspace members,** and the root `[workspace.dependencies]` declares `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"` and `sha2 = "0.10"` (the first two are there today; the scaffold plan adds the third).

**2. `crates/factory-spec/Cargo.toml` reads:**

```toml
[package]
name = "factory-spec"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "The factory spec format: parsing, validation, the split, and the scope and overlap rules. No I/O."

[features]
testkit = []

[dependencies]
serde = { workspace = true }
serde-saphyr = { version = "1.3", default-features = false, features = ["deserialize"] }
sha2 = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

The `description` line may differ; the rest must not.

**3. `crates/factory-presets/Cargo.toml` reads:**

```toml
[package]
name = "factory-presets"
edition.workspace = true
version.workspace = true
license.workspace = true
description = "Per-stack rules for the cargo and node presets: reports, test ids, protected paths, manifests, secret rules. No I/O."

[features]
testkit = []

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
quick-xml = "0.42"
toml = { version = "1", default-features = false, features = ["std", "parse", "serde"] }

[dev-dependencies]
regex = "1"
```

The `description` line may differ; the rest must not.

**4. Each crate has a `src/lib.rs`** (its content does not matter; the first task of each lane replaces it, including the skeleton's empty `testkit` module and its one trivial test). One difference from the skeleton to know about: this plan compiles the `testkit` module under `cfg(any(test, feature = "testkit"))`, not under the feature alone, so that the crate's own unit tests can use its builders without the feature being switched on.

**Why these dependencies.** Figures are from crates.io on 2026-10-04.

| Dependency | Version, features | Why this one |
|---|---|---|
| `serde` | workspace (`1`, `derive`) | Already in the workspace |
| `serde-saphyr` | `1.3`, `default-features = false`, `features = ["deserialize"]` | The YAML reader. Released 2026-09-16; about 8.9 million downloads, 6.7 million of them recent; pure Rust. `serde_yaml` has far more downloads and its last release (2024-03) is marked deprecated by its author; `serde_yaml_ng` (last release 2024-05) and `serde_norway` (2024-12) are forks of it with no release since. Checked by building this plan's code against 1.3.0: a repeated mapping key is an error by default, and a scalar is read as the type the struct asks for, so `template_version: 1` reads as the string `1`. Needs rustc 1.89 or later; the workspace toolchain is 1.93 |
| `sha2` | workspace (`0.10`) | The version already in `Cargo.lock` (today a dev-dependency of `harness-protocol`), so no second copy enters the graph. `0.11` exists; move every user together, later |
| `serde_json` | workspace (`1`) | Already in the workspace. A dev-dependency of `factory-spec`; a dependency of `factory-presets`, which reads `package.json` with it |
| `quick-xml` | `0.42` | The XML reader, used as an event stream. Released 2026-08-22; about 449 million downloads. Its API changes between minor versions (element names became `&str`, `unescape_value` was deprecated for `normalized_value`), so the requirement is the minor this plan's code was built against. **Rejected: `roxmltree` 0.21.** It was tried first and overflowed the stack while parsing about 100 nested elements in a debug build on Windows. A test report is hostile input, so a parser that recurses on document depth is not acceptable here |
| `toml` | `1`, `default-features = false`, `features = ["std", "parse", "serde"]` | The TOML reader; about 969 million downloads. Only parsing is needed, so the writer is left out |
| `regex` | `1`, dev-dependency of `factory-presets` only | The library compiles no pattern: a secret rule is data. The tests need an engine to prove each pattern compiles and matches |

**5. CI.** Please run, for each of the two crates, `cargo test -p <crate>` and also `cargo test -p <crate> --features testkit`, on Linux and on Windows. The shared vectors run inside both.

**6. For the `reqdrive` repository** (not a request to CC-COORD; recorded so the coordinating session can pass it on): a consumer runs the shared vectors with one test per crate, shown in Tasks 9 and 23, after enabling the `testkit` feature in its dev-dependencies.

**7. For the session that runs spike S6** (also not a request to CC-COORD): if the spike captures its reports by running the four test sources printed in Task 23 (`discount.rs`, `checkout.rs`, `cart.test.ts`, `cart.test.js`) at the repository-relative paths that task's index gives them, Task 25 becomes a swap of three report files. Any other sources work too; Task 25 then swaps the sources as well.

## Review Focus

Five inputs or failure modes that are likely to bite and that tests written only for the happy path would never exercise. Each has a named test in the task that owns it.

| # | Failure mode | Why it bites here | Tests, and the task that owns them |
|---|---|---|---|
| R1 | **Windows separators and letter case in paths.** A spec written on Windows says `src\cart.js`; a diff made on Linux says `src/cart.js`; a builder adds `claude.md` where `CLAUDE.md` is protected | Scope, overlap and protected-path checks are safety checks. A miss is a hole, not a cosmetic bug | Task 1 `normalize_lowers_case_and_reads_backslash_as_slash`, `check_names_each_fault` (drive letters, leading `\`); Task 2 `contains_ignores_case_and_separator_style`, `overlaps_ignores_case_and_separator_style`; Task 7 `same_layer_children_that_overlap_are_reported_once`; Task 14 `matching_ignores_case_and_separator_style`; Task 16 `the_class_and_module_path_follow_the_files_place_in_the_package`; Task 19 `only_a_cargo_manifest_is_read_as_one` |
| R2 | **CRLF line endings and a UTF-8 byte-order mark.** An editor on Windows saves the spec with CRLF or a BOM; git on Windows checks vector files out with CRLF | The signed hash is over exact bytes, so the parser must read through both without changing either. Byte offsets of test regions move if a fixture's line endings move | Task 4 `crlf_line_endings_read_the_same_as_lf`, `the_split_reads_the_same_with_crlf`; Task 5 `the_hash_covers_exact_bytes_so_line_endings_and_a_bom_change_it`, `crlf_and_a_byte_order_mark_parse_to_the_same_spec`; Task 6 `crlf_and_bom_change_the_bytes_and_the_hash_but_not_the_spec`; Tasks 9 and 23 add `vectors/.gitattributes` and the runner refuses a vector file that contains a CR; Task 12 `a_byte_order_mark_a_declaration_and_crlf_are_read_through`; Task 19 `an_unchanged_or_reformatted_cargo_manifest_adds_nothing`, `an_unchanged_or_reformatted_package_json_adds_nothing` |
| R3 | **Empty lists and empty sections.** A spec with no scope, no criteria or no children; a report with no test cases; an empty source file | An "every X satisfies Y" check passes vacuously on nothing. A spec with no criteria would pass every per-criterion check | Task 2 `an_empty_scope_contains_nothing_and_overlaps_nothing`; Task 4 `an_empty_body_and_empty_sections_mean_none`; Task 7 `no_children_is_not_a_split_and_not_a_problem`; Task 8 `an_empty_scope_is_refused_and_so_is_an_empty_test_paths`, `a_spec_with_no_criteria_is_refused`; Task 12 `a_report_with_no_test_cases_is_an_empty_list`, `an_empty_or_blank_report_is_refused`; Task 16 `files_that_are_not_rust_and_files_without_tests_have_no_ids`; Task 18 `jsx_test_files_are_refused_and_other_files_have_no_ids` |
| R4 | **A hostile or very large report, and XML entities in test names.** The report is written inside a container that ran the builder's code. Vitest writes its own ` > ` separator as `&gt;` | A crafted report must not crash or exhaust the verifier, and a name must compare equal whether or not it needed escaping | Task 12 `a_report_over_the_size_cap_is_refused_without_being_parsed`, `a_report_with_twenty_thousand_cases_reads_completely`, `nesting_up_to_the_cap_is_read_and_beyond_it_is_refused`, `a_hundred_thousand_nested_elements_are_refused_without_overflowing_the_stack`, `a_document_type_declaration_is_refused_so_no_entity_can_expand`, `xml_entities_in_names_are_decoded`; Task 22 `names_with_xml_special_characters_survive_the_round_trip` |
| R5 | **Token-shaped strings in a public repository.** The secret rules need examples that match them | GitHub push protection can refuse a push that contains a string shaped like a live key, even a made-up one. That would block the lane at its last step | Every example is stored in parts and joined at run time. Task 20 `this_source_file_matches_none_of_its_own_rules`; Task 23 `no_vector_file_matches_a_secret_rule`. This plan was checked the same way: no line of it matches a rule |

## Lane CC-SPEC

**Owns:** `crates/factory-spec/**` and nothing else.

**Reads:** `crates/fleet-core/src/*.rs` and `crates/harness-protocol/src/types.rs` for conventions. It links neither.

**Worktree and branch:** worktree `D:\MajorProjects\.swarm-wt\m0-cc-spec`, branch `feat/m0-factory-spec`, cut from `origin/factory/m0`; the pull request goes against `factory/m0`. Create them from the `command-center` checkout without touching its working tree:

```bash
git -C /d/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C /d/MajorProjects/INFRASTRUCTURE/command-center worktree add /d/MajorProjects/.swarm-wt/m0-cc-spec -b feat/m0-factory-spec origin/factory/m0
cd /d/MajorProjects/.swarm-wt/m0-cc-spec
```

Every command in this lane runs from `D:\MajorProjects\.swarm-wt\m0-cc-spec`, in Git Bash.

**Needs:** lane CC-COORD merged into `factory/m0` (the crate skeleton and its dependencies; see "Requests to the coordinator").

**Blocks:** the owner's approval of the contract crates' public types, and through it the `contracts-v0.2.0` tag that the `reqdrive` repository pins. In milestone 1, the admission, verification and store crates of the control plane and the harness's engine and payload crates all link this crate.

**Verify:**

```bash
cargo test -p factory-spec
cargo test -p factory-spec --features testkit
cargo fmt --all -- --check
cargo clippy -p factory-spec --all-targets -- -D warnings
cargo clippy -p factory-spec --all-targets --features testkit -- -D warnings
```

**Files this lane creates.**

| Path | Responsibility |
|---|---|
| `crates/factory-spec/src/lib.rs` | Module wiring and re-exports |
| `crates/factory-spec/src/path.rs` | One scope path: normal form, well-formedness, containment, overlap |
| `crates/factory-spec/src/scope.rs` | `Scope`: the four lists, grants, `contains`, `overlaps` |
| `crates/factory-spec/src/types.rs` | `Spec`, `Criterion`, `Child`, `ChildView`, `FormsIndex`, `ParseError`, `Problem` |
| `crates/factory-spec/src/body.rs` | The Markdown body: sections, lists, criteria, the split block |
| `crates/factory-spec/src/parse.rs` | `parse` (header, then body) and `sha256_hex` |
| `crates/factory-spec/src/split.rs` | `validate_split` and `child_view` |
| `crates/factory-spec/src/validate.rs` | `validate`: is the spec ready to sign |
| `crates/factory-spec/src/testkit.rs` | `SpecBuilder`, `ChildBuilder` |
| `crates/factory-spec/src/testkit/vectors.rs` | The embedded vectors and `run_all` |
| `crates/factory-spec/vectors/**` | The shared vector data |

**The file format this lane implements** (template version `1`). The header is YAML; scalars are strings; the four scope lists and the three other lists may be omitted and are then empty; an unknown key is an error. The body is read line by line:

- `## Intent`, `## Reproduction`: free text.
- `## Invariants`: a list; then optionally `### Unconfirmed` and a second list.
- `## Acceptance criteria`: one `### AC-<n>` heading per criterion (a title may follow the id), the criterion's text, then a line starting `Oracle:`; everything from there to the next heading is the oracle.
- `## Open questions`: a list.
- `## Split`: one fenced code block holding YAML, `children:` and a list; each child has `id`, `criteria`, `layer`, `depends_on` and a `scope` with the four lists.

Section names ignore case. An empty section means "none". Text before the first `## ` heading is ignored.

### Task 1: Scope paths

**Files:**
- Create: `crates/factory-spec/src/path.rs` (tests inline)
- Modify: `crates/factory-spec/src/lib.rs` (replace the skeleton's content)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum PathFault { Empty, Absolute, DotSegment, EmptySegment, Glob }` — derives `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`, serialised `snake_case`.
  - `pub(crate) fn normalize(path: &str) -> String` — `\` becomes `/`, then lower case.
  - `pub(crate) fn check(path: &str) -> Result<(), PathFault>` — is this a repository-relative file or a `/`-terminated directory prefix.
  - `pub(crate) fn entry_covers(outer: &str, inner: &str) -> bool` — both already normalised.
  - `pub(crate) fn entries_overlap(a: &str, b: &str) -> bool` — both already normalised.

- [ ] **Step 1: Confirm the skeleton is what this plan assumes**

Run:

```bash
git status --short
git branch --show-current
grep -n -A4 "^\[dependencies\]" crates/factory-spec/Cargo.toml
grep -n -A2 "^\[features\]\|^\[dev-dependencies\]" crates/factory-spec/Cargo.toml
```

Expected: a clean tree on `feat/m0-factory-spec`; under `[dependencies]` the three lines `serde = { workspace = true }`, `serde-saphyr = { version = "1.3", default-features = false, features = ["deserialize"] }` and `sha2 = { workspace = true }`; `testkit = []` under `[features]`; `serde_json = { workspace = true }` under `[dev-dependencies]`.

If the file is missing or any of those lines differs, stop and report to the coordinator. Do not edit `Cargo.toml` to make it match.

- [ ] **Step 2: Write the failing tests for scope paths**

Make `crates/factory-spec/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-spec/src/lib.rs -->
```rust
//! `factory-spec`: how a factory spec file is laid out, how it is checked before signing, how
//! it divides into child units, and which files a unit may change.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in bytes or text
//! and get plain values back. The control plane and the harness both link this crate, so the
//! two cannot disagree about whether a spec is ready or whether two units collide.

mod path;

pub use path::PathFault;
```

Create `crates/factory-spec/src/path.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/path.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowers_case_and_reads_backslash_as_slash() {
        assert_eq!(normalize("Src\\Cart.JS"), "src/cart.js");
        assert_eq!(normalize("src/discount/"), "src/discount/");
    }

    #[test]
    fn check_accepts_files_and_directory_prefixes() {
        assert_eq!(check("src/cart.js"), Ok(()));
        assert_eq!(check("src/discount/"), Ok(()));
        assert_eq!(check("src\\discount\\"), Ok(()));
        // Brackets are ordinary file-name characters (SvelteKit routes use them).
        assert_eq!(check("src/routes/[id]/+page.svelte"), Ok(()));
    }

    #[test]
    fn check_names_each_fault() {
        assert_eq!(check(""), Err(PathFault::Empty));
        assert_eq!(check("/etc/passwd"), Err(PathFault::Absolute));
        assert_eq!(check("\\server\\share"), Err(PathFault::Absolute));
        assert_eq!(check("C:/Users/x"), Err(PathFault::Absolute));
        assert_eq!(check("c:\\Users\\x"), Err(PathFault::Absolute));
        assert_eq!(check("src/../secrets/"), Err(PathFault::DotSegment));
        assert_eq!(check("./src/a.rs"), Err(PathFault::DotSegment));
        assert_eq!(check("src//a.rs"), Err(PathFault::EmptySegment));
        assert_eq!(check("src/*.rs"), Err(PathFault::Glob));
        assert_eq!(check("src/a?.rs"), Err(PathFault::Glob));
    }

    #[test]
    fn a_directory_entry_covers_what_is_under_it_and_a_file_entry_only_itself() {
        assert!(entry_covers("src/", "src/a.rs"));
        assert!(entry_covers("src/", "src/deep/a.rs"));
        assert!(entry_covers("src/", "src/deep/"));
        assert!(entry_covers("src/", "src/"));
        assert!(!entry_covers("src/", "srcx/a.rs"));
        assert!(entry_covers("src/a.rs", "src/a.rs"));
        assert!(!entry_covers("src/a.rs", "src/a.rs.bak"));
        assert!(!entry_covers("src/a/", "src/a"));
        assert!(!entry_covers("src/a/", "src/"));
        assert!(!entry_covers("src/a.rs", "src/"));
    }

    #[test]
    fn overlap_is_equal_contains_or_contained() {
        assert!(entries_overlap("src/a.rs", "src/a.rs"));
        assert!(entries_overlap("src/", "src/a.rs"));
        assert!(entries_overlap("src/a.rs", "src/"));
        assert!(entries_overlap("src/", "src/deep/"));
        assert!(!entries_overlap("src/a.rs", "src/b.rs"));
        assert!(!entries_overlap("src/a/", "src/ab/"));
        assert!(!entries_overlap("src/a", "src/ab"));
    }

    #[test]
    fn a_file_overlaps_a_directory_or_a_deeper_file_of_the_same_name() {
        assert!(entries_overlap("src/a", "src/a/"));
        assert!(entries_overlap("src/a/", "src/a"));
        assert!(entries_overlap("src/a", "src/a/b.rs"));
    }
}
```

- [ ] **Step 3: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved import `path::PathFault`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 4: Write the implementation of scope paths**

Add this to `crates/factory-spec/src/path.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/path.rs -->
```rust
//! Repository-relative paths as a scope understands them.
//!
//! An entry names one file exactly (`src/cart.js`), or a whole directory by a prefix that
//! ends in `/` (`src/discount/`). Wildcards are not supported. Letter case is ignored and a
//! `\` is read as `/`, so a scope written on Windows and a diff produced on Linux agree.

use serde::{Deserialize, Serialize};

/// Why a path is not usable in a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathFault {
    /// The path is the empty string.
    Empty,
    /// The path starts at a root (`/x`, `\x`) or names a drive (`C:/x`).
    Absolute,
    /// A segment is `.` or `..`, so the text does not say where the path really points.
    DotSegment,
    /// Two separators in a row (`a//b`).
    EmptySegment,
    /// The path contains `*` or `?`. Scope has no globs.
    Glob,
}

/// The form two paths are compared in: `/` separators, lower case.
pub(crate) fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// Check that `path` is a repository-relative file or a `/`-terminated directory prefix.
pub(crate) fn check(path: &str) -> Result<(), PathFault> {
    let p = path.replace('\\', "/");
    if p.is_empty() {
        return Err(PathFault::Empty);
    }
    if p.starts_with('/') || p.as_bytes().get(1) == Some(&b':') {
        return Err(PathFault::Absolute);
    }
    if p.contains('*') || p.contains('?') {
        return Err(PathFault::Glob);
    }
    let stem = p.strip_suffix('/').unwrap_or(&p);
    for segment in stem.split('/') {
        match segment {
            "" => return Err(PathFault::EmptySegment),
            "." | ".." => return Err(PathFault::DotSegment),
            _ => {}
        }
    }
    Ok(())
}

/// True if scope entry `outer` permits everything `inner` names, where `inner` is a file or
/// another scope entry. Both arguments are already normalised.
pub(crate) fn entry_covers(outer: &str, inner: &str) -> bool {
    if outer.ends_with('/') {
        inner.starts_with(outer)
    } else {
        outer == inner
    }
}

/// True if the two scope entries are equal, or one lies under the other.
///
/// The trailing `/` is ignored here on purpose: a file `a/b` overlaps both the directory
/// `a/b/` and the file `a/b/c`, because one tree cannot hold both.
/// Both arguments are already normalised.
pub(crate) fn entries_overlap(a: &str, b: &str) -> bool {
    let a = a.strip_suffix('/').unwrap_or(a);
    let b = b.strip_suffix('/').unwrap_or(b);
    let under = |child: &str, parent: &str| {
        child.len() > parent.len()
            && child.starts_with(parent)
            && child.as_bytes()[parent.len()] == b'/'
    };
    a == b || under(a, b) || under(b, a)
}
```

Brackets are deliberately not glob characters: `src/routes/[id]/+page.svelte` is an ordinary SvelteKit file name. Only `*` and `?` are refused, and neither can appear in a Windows file name anyway.

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 6 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add crates/factory-spec/src/lib.rs crates/factory-spec/src/path.rs
git commit -m "feat(factory-spec): scope path normal form, well-formedness and overlap"
```


### Task 2: `Scope`

**Files:**
- Create: `crates/factory-spec/src/scope.rs` (tests inline)
- Modify: `crates/factory-spec/src/lib.rs`

**Interfaces:**
- Consumes: `path::{normalize, check, entry_covers, entries_overlap}` (Task 1).
- Produces:
  - `pub struct Scope { pub touched_files: Vec<String>, pub create_paths: Vec<String>, pub test_paths: Vec<String>, pub touched_tests: Vec<String> }` — derives `Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize`; every list defaults to empty; an unknown key is refused.
  - `pub fn with_grants(&self, grants: &[String]) -> Scope`
  - `pub fn contains(&self, path: &str) -> bool`
  - `pub fn overlaps(&self, other: &Scope) -> bool`
  - `pub(crate) fn entries(&self) -> impl Iterator<Item = &String>`
  - `pub(crate) fn is_empty(&self) -> bool`
  - `pub(crate) fn covers(&self, inner: &Scope) -> Result<(), String>` — `Err` carries the first entry of `inner` left outside.

- [ ] **Step 1: Write the failing tests for `Scope`**

Make `crates/factory-spec/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-spec/src/lib.rs -->
```rust
//! `factory-spec`: how a factory spec file is laid out, how it is checked before signing, how
//! it divides into child units, and which files a unit may change.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in bytes or text
//! and get plain values back. The control plane and the harness both link this crate, so the
//! two cannot disagree about whether a spec is ready or whether two units collide.

mod path;
mod scope;

pub use path::PathFault;
pub use scope::Scope;
```

Create `crates/factory-spec/src/scope.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/scope.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn scope(touched: &[&str], create: &[&str], tests: &[&str], touched_tests: &[&str]) -> Scope {
        let own = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect();
        Scope {
            touched_files: own(touched),
            create_paths: own(create),
            test_paths: own(tests),
            touched_tests: own(touched_tests),
        }
    }

    #[test]
    fn contains_reads_all_four_lists() {
        let s = scope(
            &["src/cart.js"],
            &["src/discount/"],
            &["test/"],
            &["old/cart.test.js"],
        );
        assert!(s.contains("src/cart.js"));
        assert!(s.contains("src/discount/percent.js"));
        assert!(s.contains("test/discount.test.js"));
        assert!(s.contains("old/cart.test.js"));
        assert!(!s.contains("src/checkout.js"));
        assert!(!s.contains("package.json"));
    }

    #[test]
    fn contains_ignores_case_and_separator_style() {
        let s = scope(&["src/Cart.js"], &["src\\discount\\"], &[], &[]);
        assert!(s.contains("SRC/cart.JS"));
        assert!(s.contains("src\\cart.js"));
        assert!(s.contains("src/Discount/percent.js"));
        assert!(s.contains("src\\discount\\percent.js"));
    }

    #[test]
    fn contains_refuses_paths_that_climb_or_are_absolute() {
        let s = scope(&[], &["src/"], &[], &[]);
        assert!(!s.contains("src/../secrets.env"));
        assert!(!s.contains("/src/a.rs"));
        assert!(!s.contains("C:\\repo\\src\\a.rs"));
        assert!(!s.contains(""));
    }

    #[test]
    fn an_empty_scope_contains_nothing_and_overlaps_nothing() {
        let empty = Scope::default();
        let full = scope(&["src/a.rs"], &[], &[], &[]);
        assert!(empty.is_empty());
        assert!(!full.is_empty());
        assert!(!empty.contains("src/a.rs"));
        assert!(!empty.overlaps(&full));
        assert!(!full.overlaps(&empty));
        assert!(!empty.overlaps(&empty));
    }

    #[test]
    fn with_grants_widens_the_union_and_leaves_the_original_alone() {
        let s = scope(&["src/cart.js"], &[], &["test/"], &[]);
        let widened = s.with_grants(&["src/tax.js".to_string(), "docs/".to_string()]);
        assert!(widened.contains("src/tax.js"));
        assert!(widened.contains("docs/tax.md"));
        assert!(widened.contains("src/cart.js"));
        assert!(!s.contains("src/tax.js"));
        assert_eq!(widened.test_paths, s.test_paths);
        assert_eq!(s.with_grants(&[]), s);
    }

    #[test]
    fn overlaps_across_different_lists_and_in_both_directions() {
        let a = scope(&["src/cart.js"], &[], &["test/a/"], &[]);
        let b = scope(&[], &["src/"], &["test/b/"], &[]);
        let c = scope(&["src/tax.js"], &[], &["test/c/"], &[]);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(b.overlaps(&c));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn overlaps_ignores_case_and_separator_style() {
        let a = scope(&["SRC\\Cart.js"], &[], &[], &[]);
        let b = scope(&["src/cart.JS"], &[], &[], &[]);
        assert!(a.overlaps(&b));
    }

    #[test]
    fn covers_names_the_first_entry_left_outside() {
        let parent = scope(&["src/cart.js"], &["src/discount/"], &["test/"], &[]);
        let inside = scope(
            &["src/cart.js"],
            &["src/discount/percent/"],
            &["test/x.test.js"],
            &[],
        );
        let outside = scope(&["src/cart.js", "src/tax.js"], &[], &[], &[]);
        let wider = scope(&[], &["src/"], &[], &[]);
        assert_eq!(parent.covers(&inside), Ok(()));
        assert_eq!(parent.covers(&outside), Err("src/tax.js".to_string()));
        assert_eq!(parent.covers(&wider), Err("src/".to_string()));
        assert_eq!(parent.covers(&Scope::default()), Ok(()));
    }

    #[test]
    fn scope_reads_from_yaml_with_missing_lists_empty_and_refuses_unknown_keys() {
        let s: Scope = serde_saphyr::from_str("touched_files: [src/a.rs]\n").unwrap();
        assert_eq!(s, scope(&["src/a.rs"], &[], &[], &[]));
        assert!(serde_saphyr::from_str::<Scope>("touched_file: [src/a.rs]\n").is_err());
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved import `scope::Scope`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of `Scope`**

Add this to `crates/factory-spec/src/scope.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/scope.rs -->
```rust
//! A unit's scope: the files it may change.

use crate::path;
use serde::{Deserialize, Serialize};

/// The four path lists of a spec or of one child. Taken together they are everything the
/// unit is allowed to change.
///
/// Each entry is one file, or a directory given as a prefix ending in `/`. Removing a file
/// counts as changing it, and a rename counts as removing the old path and adding the new
/// one, so a caller checks **both** paths of a rename with [`Scope::contains`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    /// Existing files the unit may edit or delete.
    #[serde(default)]
    pub touched_files: Vec<String>,
    /// Files or directories the unit may create.
    #[serde(default)]
    pub create_paths: Vec<String>,
    /// Where the unit's new tests go.
    #[serde(default)]
    pub test_paths: Vec<String>,
    /// Existing tests the spec requires to change.
    #[serde(default)]
    pub touched_tests: Vec<String>,
}

impl Scope {
    /// Every entry of the four lists, in list order.
    pub(crate) fn entries(&self) -> impl Iterator<Item = &String> {
        self.touched_files
            .iter()
            .chain(&self.create_paths)
            .chain(&self.test_paths)
            .chain(&self.touched_tests)
    }

    /// True if the four lists are all empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.entries().next().is_none()
    }

    /// This scope widened by scope grants. Grants join `touched_files`: a grant permits a
    /// change and carries no test meaning.
    pub fn with_grants(&self, grants: &[String]) -> Scope {
        let mut widened = self.clone();
        widened.touched_files.extend(grants.iter().cloned());
        widened
    }

    /// True if a change to the file `path` is inside this scope: `path` is an entry, or lies
    /// under a `/`-terminated entry. A `path` that is not a well-formed relative file path
    /// (empty, absolute, with a `.` or `..` segment) is never inside.
    pub fn contains(&self, path: &str) -> bool {
        if path::check(path).is_err() {
            return false;
        }
        let file = path::normalize(path);
        self.entries()
            .any(|entry| path::entry_covers(&path::normalize(entry), &file))
    }

    /// `Ok` if every entry of `inner` is inside this scope; otherwise the first entry of
    /// `inner` that is not.
    pub(crate) fn covers(&self, inner: &Scope) -> Result<(), String> {
        for entry in inner.entries() {
            let wanted = path::normalize(entry);
            let covered = self
                .entries()
                .any(|outer| path::entry_covers(&path::normalize(outer), &wanted));
            if !covered {
                return Err(entry.clone());
            }
        }
        Ok(())
    }

    /// True if the two scopes could touch the same file: some entry here is the same path as
    /// an entry of `other`, or sits above or below it in the tree. Letter case and the
    /// direction of the slashes are ignored.
    pub fn overlaps(&self, other: &Scope) -> bool {
        let theirs: Vec<String> = other.entries().map(|e| path::normalize(e)).collect();
        self.entries().any(|mine| {
            let mine = path::normalize(mine);
            theirs.iter().any(|t| path::entries_overlap(&mine, t))
        })
    }
}
```

`contains` answers for one changed file. For a rename the caller asks twice, once for the old path and once for the new one; for a deletion it asks for the deleted path.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 15 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-spec/src/lib.rs crates/factory-spec/src/scope.rs
git commit -m "feat(factory-spec): Scope with grants, containment and overlap"
```


### Task 3: The spec's types, `ParseError` and `Problem`

**Files:**
- Create: `crates/factory-spec/src/types.rs` (tests inline)
- Modify: `crates/factory-spec/src/lib.rs`

**Interfaces:**
- Consumes: `PathFault` (Task 1), `Scope` (Task 2).
- Produces (all `pub`):
  - `const TEMPLATE_VERSION: &str = "1"`
  - `enum Kind { Feature, Bugfix, Chore }` (`snake_case`); `enum Tier { T1, T2, T3 }` (lower case)
  - `struct Criterion { id: String, text: String, oracle: String }`
  - `struct Child { id: String, criteria: Vec<String>, scope: Scope, layer: u32, depends_on: Vec<String> }` — `layer` and `id` are required when read from YAML; unknown keys are refused
  - `struct ChildView { id: String, criteria: Vec<Criterion>, scope: Scope, layer: u32, depends_on: Vec<String> }`
  - `struct Spec { id, work_item, kind: Kind, tier: Tier, scope: Scope, permitted_dependencies, expected_red, forms_affected: Vec<String>, source_hash, template_version: String, intent: String, reproduction: Option<String>, invariants, unconfirmed_invariants: Vec<String>, criteria: Vec<Criterion>, open_questions: Vec<String>, children: Vec<Child> }`
  - `struct FormsIndex` with `fn new() -> Self`, `fn insert(&mut self, form: &str, path: &str)`, `fn entries(&self) -> impl Iterator<Item = (&str, &str)>`
  - `enum ParseError { NotUtf8, MissingHeader, UnterminatedHeader, Header(String), UnsupportedTemplate { found: String }, UnknownSection { heading: String, line: usize }, DuplicateSection { heading: String, line: usize }, UnterminatedFence { line: usize }, Split(String) }` with `fn code(&self) -> &'static str`
  - `enum Problem { OpenQuestions { count: usize }, UnconfirmedInvariants { count: usize }, EmptyScope, EmptyTestPaths, PathMalformed { path: String, fault: PathFault }, NoCriteria, CriterionWithoutId { position: usize }, CriterionWithoutOracle { criterion: String }, DuplicateCriterionId { id: String }, FormNotDeclared { form: String, path: String }, SplitTooSmall, DuplicateChildId { id: String }, CriterionUnassigned { criterion: String }, CriterionAssignedTwice { criterion: String, children: Vec<String> }, UnknownCriterion { child: String, criterion: String }, ChildTestPathsEmpty { child: String }, ChildScopeOutsideParent { child: String, path: String }, ChildrenOverlap { layer: u32, a: String, b: String }, UnknownDependency { child: String, dependency: String }, DependencyNotEarlier { child: String, dependency: String }, UnknownChild { child: String } }` — serialised with the tag `problem` and `snake_case` names, with `fn code(&self) -> &'static str` equal to that tag
  - `Display` and `std::error::Error` for both error enums.

- [ ] **Step 1: Write the failing tests for the types and the two error enums**

Make `crates/factory-spec/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-spec/src/lib.rs -->
```rust
//! `factory-spec`: how a factory spec file is laid out, how it is checked before signing, how
//! it divides into child units, and which files a unit may change.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in bytes or text
//! and get plain values back. The control plane and the harness both link this crate, so the
//! two cannot disagree about whether a spec is ready or whether two units collide.

mod path;
mod scope;
mod types;

pub use path::PathFault;
pub use scope::Scope;
pub use types::{
    Child, ChildView, Criterion, FormsIndex, Kind, ParseError, Problem, Spec, Tier,
    TEMPLATE_VERSION,
};
```

Create `crates/factory-spec/src/types.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/types.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_and_tier_read_their_wire_strings() {
        assert_eq!(
            serde_saphyr::from_str::<Kind>("bugfix").unwrap(),
            Kind::Bugfix
        );
        assert_eq!(
            serde_saphyr::from_str::<Kind>("feature").unwrap(),
            Kind::Feature
        );
        assert_eq!(
            serde_saphyr::from_str::<Kind>("chore").unwrap(),
            Kind::Chore
        );
        assert_eq!(serde_saphyr::from_str::<Tier>("t2").unwrap(), Tier::T2);
        assert!(serde_saphyr::from_str::<Tier>("t4").is_err());
        assert!(serde_saphyr::from_str::<Kind>("refactor").is_err());
    }

    #[test]
    fn a_child_reads_from_yaml_with_optional_lists_empty() {
        let child: Child = serde_saphyr::from_str(
            "id: pricing\ncriteria: [AC-1]\nlayer: 0\nscope:\n  test_paths: [test/pricing/]\n",
        )
        .unwrap();
        assert_eq!(child.id, "pricing");
        assert_eq!(child.criteria, vec!["AC-1".to_string()]);
        assert_eq!(child.layer, 0);
        assert!(child.depends_on.is_empty());
        assert_eq!(child.scope.test_paths, vec!["test/pricing/".to_string()]);
    }

    #[test]
    fn a_child_refuses_an_unknown_key_and_a_missing_layer() {
        assert!(serde_saphyr::from_str::<Child>("id: a\nlayer: 0\nlayers: 1\n").is_err());
        assert!(serde_saphyr::from_str::<Child>("id: a\n").is_err());
    }

    #[test]
    fn forms_index_lists_pairs_ordered_by_form() {
        let mut forms = FormsIndex::new();
        forms.insert("pricing", "src/pricing/api.js");
        forms.insert("cart", "src/cart/");
        forms.insert("pricing", "src/pricing/types.js");
        let pairs: Vec<(&str, &str)> = forms.entries().collect();
        assert_eq!(
            pairs,
            vec![
                ("cart", "src/cart/"),
                ("pricing", "src/pricing/api.js"),
                ("pricing", "src/pricing/types.js"),
            ]
        );
        assert_eq!(FormsIndex::new().entries().count(), 0);
    }

    #[test]
    fn every_problem_code_is_distinct_and_display_is_never_empty() {
        let all = vec![
            Problem::OpenQuestions { count: 1 },
            Problem::UnconfirmedInvariants { count: 1 },
            Problem::EmptyScope,
            Problem::EmptyTestPaths,
            Problem::PathMalformed {
                path: "/x".into(),
                fault: PathFault::Absolute,
            },
            Problem::NoCriteria,
            Problem::CriterionWithoutId { position: 1 },
            Problem::CriterionWithoutOracle {
                criterion: "AC-1".into(),
            },
            Problem::DuplicateCriterionId { id: "AC-1".into() },
            Problem::FormNotDeclared {
                form: "cart".into(),
                path: "src/cart/".into(),
            },
            Problem::SplitTooSmall,
            Problem::DuplicateChildId { id: "a".into() },
            Problem::CriterionUnassigned {
                criterion: "AC-1".into(),
            },
            Problem::CriterionAssignedTwice {
                criterion: "AC-1".into(),
                children: vec!["a".into(), "b".into()],
            },
            Problem::UnknownCriterion {
                child: "a".into(),
                criterion: "AC-9".into(),
            },
            Problem::ChildTestPathsEmpty { child: "a".into() },
            Problem::ChildScopeOutsideParent {
                child: "a".into(),
                path: "x/".into(),
            },
            Problem::ChildrenOverlap {
                layer: 0,
                a: "a".into(),
                b: "b".into(),
            },
            Problem::UnknownDependency {
                child: "a".into(),
                dependency: "z".into(),
            },
            Problem::DependencyNotEarlier {
                child: "a".into(),
                dependency: "b".into(),
            },
            Problem::UnknownChild { child: "z".into() },
        ];
        let mut codes: Vec<&str> = all.iter().map(Problem::code).collect();
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), before, "two problems share a code");
        for problem in &all {
            assert!(!problem.to_string().is_empty());
            // The code is the tag the problem serialises with, and it reads back.
            let json = serde_json::to_value(problem).unwrap();
            assert_eq!(json["problem"], problem.code());
            let back: Problem = serde_json::from_value(json).unwrap();
            assert_eq!(&back, problem);
        }
    }

    #[test]
    fn a_problem_serialises_flat_with_snake_case_fields() {
        let problem = Problem::PathMalformed {
            path: "src/*.js".into(),
            fault: PathFault::Glob,
        };
        assert_eq!(
            serde_json::to_value(&problem).unwrap(),
            serde_json::json!({"problem": "path_malformed", "path": "src/*.js", "fault": "glob"})
        );
    }

    #[test]
    fn parse_error_codes_are_distinct() {
        let all = [
            ParseError::NotUtf8,
            ParseError::MissingHeader,
            ParseError::UnterminatedHeader,
            ParseError::Header("x".into()),
            ParseError::UnsupportedTemplate { found: "9".into() },
            ParseError::UnknownSection {
                heading: "x".into(),
                line: 1,
            },
            ParseError::DuplicateSection {
                heading: "x".into(),
                line: 1,
            },
            ParseError::UnterminatedFence { line: 1 },
            ParseError::Split("x".into()),
        ];
        let mut codes: Vec<&str> = all.iter().map(ParseError::code).collect();
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), before);
        for error in &all {
            assert!(!error.to_string().is_empty());
        }
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved imports `types::Child`, `types::ChildView`, `types::Criterion`, `types::FormsIndex`, `types::Kind`, `types::ParseError`, `types::Problem`, `types::Spec`, `types::Tier`, `types::TEMPLATE_VERSION`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the types and the two error enums**

Add this to `crates/factory-spec/src/types.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/types.rs -->
```rust
//! The parsed spec, and the two ways reading one can go wrong: it does not parse
//! ([`ParseError`]), or it parses and is not ready to sign ([`Problem`]).

use crate::path::PathFault;
use crate::scope::Scope;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The template version this crate reads and the testkit writes.
pub const TEMPLATE_VERSION: &str = "1";

/// What kind of change a spec asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Feature,
    Bugfix,
    Chore,
}

/// The autonomy tier. Mirrors `fleet-core`'s `Tier` on the wire (`t1`/`t2`/`t3`); this crate
/// links neither `fleet-core` nor `harness-protocol`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    T1,
    T2,
    T3,
}

/// One acceptance criterion. `id` is empty when the heading carried no well-formed id, and
/// `oracle` is empty when the criterion has no `Oracle:` line; [`crate::validate`] refuses both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    pub id: String,
    pub text: String,
    pub oracle: String,
}

/// One child unit of a split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Child {
    pub id: String,
    /// Ids of the parent's criteria this child owns.
    #[serde(default)]
    pub criteria: Vec<String>,
    #[serde(default)]
    pub scope: Scope,
    /// Children in one layer run in parallel; layer 0 runs first.
    pub layer: u32,
    /// Ids of children in strictly earlier layers.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// A child as a unit sees it: its criteria in full and its scope, looked up in the parent
/// spec. A child has no file of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildView {
    pub id: String,
    pub criteria: Vec<Criterion>,
    pub scope: Scope,
    pub layer: u32,
    pub depends_on: Vec<String>,
}

/// A parsed spec file: the header fields and the body sections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spec {
    pub id: String,
    /// The work item this spec was drafted from, as an opaque reference (`owner/repo#12`).
    pub work_item: String,
    pub kind: Kind,
    pub tier: Tier,
    /// `touched_files`, `create_paths`, `test_paths` and `touched_tests` from the header.
    pub scope: Scope,
    pub permitted_dependencies: Vec<String>,
    pub expected_red: Vec<String>,
    pub forms_affected: Vec<String>,
    /// The hash of the work-item text the spec was drafted from. Opaque to this crate.
    pub source_hash: String,
    pub template_version: String,
    pub intent: String,
    /// Present only when the file has a `## Reproduction` section.
    pub reproduction: Option<String>,
    pub invariants: Vec<String>,
    pub unconfirmed_invariants: Vec<String>,
    pub criteria: Vec<Criterion>,
    pub open_questions: Vec<String>,
    /// The split. Empty when the spec runs as one unit.
    pub children: Vec<Child>,
}

/// Which files are which Form's interface. The caller builds it from the repository's Forms
/// registry; this crate never reads a registry itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormsIndex {
    interface_files: BTreeMap<String, Vec<String>>,
}

impl FormsIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `path` (a file, or a `/`-terminated directory) is part of `form`'s interface.
    pub fn insert(&mut self, form: &str, path: &str) {
        self.interface_files
            .entry(form.to_string())
            .or_default()
            .push(path.to_string());
    }

    /// Every `(form, interface path)` pair, ordered by form name.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.interface_files
            .iter()
            .flat_map(|(form, paths)| paths.iter().map(move |p| (form.as_str(), p.as_str())))
    }
}

/// The file is not a spec this crate can read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The bytes are not UTF-8.
    NotUtf8,
    /// The first line is not `---`.
    MissingHeader,
    /// The header's closing `---` line is missing.
    UnterminatedHeader,
    /// The header is not valid YAML, lacks a required field, has an unknown field, or has a
    /// field of the wrong type. The text is the YAML reader's message.
    Header(String),
    /// The header names a template version this crate does not read.
    UnsupportedTemplate { found: String },
    /// A `## ` heading (or a `### ` heading where none is allowed) that the template does not
    /// define. Refused so that a mistyped "Open questions" cannot hide open questions.
    UnknownSection { heading: String, line: usize },
    /// The same section appears twice.
    DuplicateSection { heading: String, line: usize },
    /// A fenced code block opened on this line is never closed, so it would swallow every
    /// section after it.
    UnterminatedFence { line: usize },
    /// The split section is not one fenced YAML block, or the block does not read as a list
    /// of children.
    Split(String),
}

impl ParseError {
    /// A stable short name, used by the shared vectors.
    pub fn code(&self) -> &'static str {
        match self {
            ParseError::NotUtf8 => "not_utf8",
            ParseError::MissingHeader => "missing_header",
            ParseError::UnterminatedHeader => "unterminated_header",
            ParseError::Header(_) => "header",
            ParseError::UnsupportedTemplate { .. } => "unsupported_template",
            ParseError::UnknownSection { .. } => "unknown_section",
            ParseError::DuplicateSection { .. } => "duplicate_section",
            ParseError::UnterminatedFence { .. } => "unterminated_fence",
            ParseError::Split(_) => "split",
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NotUtf8 => write!(f, "the spec is not UTF-8 text"),
            ParseError::MissingHeader => write!(f, "the spec does not start with a `---` line"),
            ParseError::UnterminatedHeader => write!(f, "the header has no closing `---` line"),
            ParseError::Header(detail) => write!(f, "the header does not read: {detail}"),
            ParseError::UnsupportedTemplate { found } => write!(
                f,
                "template_version {found:?} is not supported (this reader reads {TEMPLATE_VERSION:?})"
            ),
            ParseError::UnknownSection { heading, line } => {
                write!(f, "line {line}: unknown section {heading:?}")
            }
            ParseError::DuplicateSection { heading, line } => {
                write!(f, "line {line}: section {heading:?} appears twice")
            }
            ParseError::UnterminatedFence { line } => {
                write!(f, "line {line}: this code fence is never closed")
            }
            ParseError::Split(detail) => write!(f, "the split does not read: {detail}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// One reason a parsed spec is not ready to sign, or a split is not valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum Problem {
    /// Open questions remain.
    OpenQuestions { count: usize },
    /// Invariants nobody has confirmed remain.
    UnconfirmedInvariants { count: usize },
    /// All four scope lists are empty.
    EmptyScope,
    /// `test_paths` is empty, so the unit has nowhere to put its tests.
    EmptyTestPaths,
    /// A scope entry is not a relative file or a `/`-terminated directory prefix.
    PathMalformed { path: String, fault: PathFault },
    /// The spec has no acceptance criteria at all.
    NoCriteria,
    /// The criterion at this position (from 1) has no well-formed id.
    CriterionWithoutId { position: usize },
    /// The criterion (its id, or `#<position>` when it has none) has no oracle.
    CriterionWithoutOracle { criterion: String },
    /// Two criteria share an id.
    DuplicateCriterionId { id: String },
    /// A scope entry reaches a Form's interface file and `forms_affected` does not name the Form.
    FormNotDeclared { form: String, path: String },
    /// The split lists exactly one child. Two or more make a split; none means one unit.
    SplitTooSmall,
    /// Two children share an id.
    DuplicateChildId { id: String },
    /// A parent criterion belongs to no child.
    CriterionUnassigned { criterion: String },
    /// A parent criterion belongs to more than one child.
    CriterionAssignedTwice {
        criterion: String,
        children: Vec<String>,
    },
    /// A child names a criterion the parent does not have.
    UnknownCriterion { child: String, criterion: String },
    /// A child has no `test_paths`. Each child is a full unit and writes its own tests.
    ChildTestPathsEmpty { child: String },
    /// A child's scope entry is outside the parent's scope.
    ChildScopeOutsideParent { child: String, path: String },
    /// Two children in the same layer have overlapping scopes.
    ChildrenOverlap { layer: u32, a: String, b: String },
    /// A child depends on an id that is not a child.
    UnknownDependency { child: String, dependency: String },
    /// A child depends on a child in the same or a later layer.
    DependencyNotEarlier { child: String, dependency: String },
    /// No child has this id.
    UnknownChild { child: String },
}

impl Problem {
    /// A stable short name: the `problem` tag this value serialises with.
    pub fn code(&self) -> &'static str {
        match self {
            Problem::OpenQuestions { .. } => "open_questions",
            Problem::UnconfirmedInvariants { .. } => "unconfirmed_invariants",
            Problem::EmptyScope => "empty_scope",
            Problem::EmptyTestPaths => "empty_test_paths",
            Problem::PathMalformed { .. } => "path_malformed",
            Problem::NoCriteria => "no_criteria",
            Problem::CriterionWithoutId { .. } => "criterion_without_id",
            Problem::CriterionWithoutOracle { .. } => "criterion_without_oracle",
            Problem::DuplicateCriterionId { .. } => "duplicate_criterion_id",
            Problem::FormNotDeclared { .. } => "form_not_declared",
            Problem::SplitTooSmall => "split_too_small",
            Problem::DuplicateChildId { .. } => "duplicate_child_id",
            Problem::CriterionUnassigned { .. } => "criterion_unassigned",
            Problem::CriterionAssignedTwice { .. } => "criterion_assigned_twice",
            Problem::UnknownCriterion { .. } => "unknown_criterion",
            Problem::ChildTestPathsEmpty { .. } => "child_test_paths_empty",
            Problem::ChildScopeOutsideParent { .. } => "child_scope_outside_parent",
            Problem::ChildrenOverlap { .. } => "children_overlap",
            Problem::UnknownDependency { .. } => "unknown_dependency",
            Problem::DependencyNotEarlier { .. } => "dependency_not_earlier",
            Problem::UnknownChild { .. } => "unknown_child",
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::OpenQuestions { count } => write!(f, "{count} open question(s) remain"),
            Problem::UnconfirmedInvariants { count } => {
                write!(f, "{count} unconfirmed invariant(s) remain")
            }
            Problem::EmptyScope => write!(f, "the scope is empty"),
            Problem::EmptyTestPaths => write!(f, "test_paths is empty"),
            Problem::PathMalformed { path, fault } => {
                write!(f, "scope entry {path:?} is not usable: {fault:?}")
            }
            Problem::NoCriteria => write!(f, "the spec has no acceptance criteria"),
            Problem::CriterionWithoutId { position } => {
                write!(f, "criterion {position} has no id of the form AC-<number>")
            }
            Problem::CriterionWithoutOracle { criterion } => {
                write!(f, "criterion {criterion} has no oracle")
            }
            Problem::DuplicateCriterionId { id } => write!(f, "criterion id {id} is used twice"),
            Problem::FormNotDeclared { form, path } => write!(
                f,
                "{path:?} reaches the interface of Form {form:?}, which forms_affected does not name"
            ),
            Problem::SplitTooSmall => write!(f, "the split lists one child; a split needs two"),
            Problem::DuplicateChildId { id } => write!(f, "child id {id:?} is used twice"),
            Problem::CriterionUnassigned { criterion } => {
                write!(f, "criterion {criterion} belongs to no child")
            }
            Problem::CriterionAssignedTwice {
                criterion,
                children,
            } => write!(
                f,
                "criterion {criterion} belongs to more than one child: {}",
                children.join(", ")
            ),
            Problem::UnknownCriterion { child, criterion } => {
                write!(f, "child {child:?} names unknown criterion {criterion}")
            }
            Problem::ChildTestPathsEmpty { child } => {
                write!(f, "child {child:?} has no test_paths")
            }
            Problem::ChildScopeOutsideParent { child, path } => {
                write!(f, "child {child:?}: {path:?} is outside the parent's scope")
            }
            Problem::ChildrenOverlap { layer, a, b } => {
                write!(f, "children {a:?} and {b:?} overlap in layer {layer}")
            }
            Problem::UnknownDependency { child, dependency } => {
                write!(f, "child {child:?} depends on unknown child {dependency:?}")
            }
            Problem::DependencyNotEarlier { child, dependency } => write!(
                f,
                "child {child:?} depends on {dependency:?}, which is not in an earlier layer"
            ),
            Problem::UnknownChild { child } => write!(f, "the split has no child {child:?}"),
        }
    }
}

impl std::error::Error for Problem {}
```

`work_item` is an opaque reference such as `owner/repo#12`: this crate links neither `fleet-core` nor `harness-protocol`, so it does not parse it. `source_hash` is opaque for the same reason.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 22 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-spec/src/lib.rs crates/factory-spec/src/types.rs
git commit -m "feat(factory-spec): Spec, Criterion, Child, FormsIndex, ParseError and Problem"
```


### Task 4: The Markdown body

**Files:**
- Create: `crates/factory-spec/src/body.rs` (tests inline)
- Modify: `crates/factory-spec/src/lib.rs`

**Interfaces:**
- Consumes: `Child`, `Criterion`, `ParseError` (Task 3).
- Produces (all `pub(crate)`):
  - `struct Body { intent: String, reproduction: Option<String>, invariants: Vec<String>, unconfirmed_invariants: Vec<String>, criteria: Vec<Criterion>, open_questions: Vec<String>, children: Vec<Child> }` — derives `Debug, Default, PartialEq, Eq`
  - `fn parse_body(text: &str, first_line: usize) -> Result<Body, ParseError>` — `first_line` is the 1-based file line of `text`'s first line, so errors name file lines
  - `fn is_criterion_id(s: &str) -> bool` — `AC-` then a decimal number with no leading zero

- [ ] **Step 1: Write the failing tests for the body reader**

Make `crates/factory-spec/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-spec/src/lib.rs -->
```rust
//! `factory-spec`: how a factory spec file is laid out, how it is checked before signing, how
//! it divides into child units, and which files a unit may change.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in bytes or text
//! and get plain values back. The control plane and the harness both link this crate, so the
//! two cannot disagree about whether a spec is ready or whether two units collide.

mod body;
mod path;
mod scope;
mod types;

pub use path::PathFault;
pub use scope::Scope;
pub use types::{
    Child, ChildView, Criterion, FormsIndex, Kind, ParseError, Problem, Spec, Tier,
    TEMPLATE_VERSION,
};
```

Create `crates/factory-spec/src/body.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/body.rs -->
````rust
#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = "\
# Percent discount codes

## Intent

A cart accepts one percent discount code.

```js
## not a heading
```

## Reproduction

Apply SAVE10 twice; the total drops twice.

## Invariants

- A total is never negative.
- Prices are integers,
  in cents.

### Unconfirmed

- Codes are case-insensitive.

## Acceptance criteria

### AC-1

A 10% code on a 100.00 cart totals 90.00.

Oracle: a unit test calls `total()` with the code applied
and compares the result.

### AC-2: Unknown codes

An unknown code leaves the total unchanged.

**Oracle:** a unit test applies `NOPE` and compares the total.

## Open questions

- Do codes stack?
* Is there a minimum spend?
";

    #[test]
    fn reads_every_section_of_a_full_body() {
        let body = parse_body(FULL, 1).unwrap();
        assert_eq!(
            body.intent,
            "A cart accepts one percent discount code.\n\n```js\n## not a heading\n```"
        );
        assert_eq!(
            body.reproduction.as_deref(),
            Some("Apply SAVE10 twice; the total drops twice.")
        );
        assert_eq!(
            body.invariants,
            vec![
                "A total is never negative.".to_string(),
                "Prices are integers, in cents.".to_string(),
            ]
        );
        assert_eq!(
            body.unconfirmed_invariants,
            vec!["Codes are case-insensitive.".to_string()]
        );
        assert_eq!(
            body.open_questions,
            vec![
                "Do codes stack?".to_string(),
                "Is there a minimum spend?".to_string(),
            ]
        );
        assert!(body.children.is_empty());
    }

    #[test]
    fn reads_criteria_with_id_text_and_oracle() {
        let body = parse_body(FULL, 1).unwrap();
        assert_eq!(
            body.criteria,
            vec![
                Criterion {
                    id: "AC-1".to_string(),
                    text: "A 10% code on a 100.00 cart totals 90.00.".to_string(),
                    oracle: "a unit test calls `total()` with the code applied\nand compares the result."
                        .to_string(),
                },
                Criterion {
                    id: "AC-2".to_string(),
                    text: "Unknown codes\nAn unknown code leaves the total unchanged.".to_string(),
                    oracle: "a unit test applies `NOPE` and compares the total.".to_string(),
                },
            ]
        );
    }

    #[test]
    fn crlf_line_endings_read_the_same_as_lf() {
        let crlf = FULL.replace('\n', "\r\n");
        assert_eq!(parse_body(&crlf, 1).unwrap(), parse_body(FULL, 1).unwrap());
    }

    #[test]
    fn an_empty_body_and_empty_sections_mean_none() {
        assert_eq!(parse_body("", 1).unwrap(), Body::default());
        let body = parse_body(
            "## Intent\n\n## Invariants\n\n### Unconfirmed\n\n## Acceptance criteria\n\n## Open questions\n\n## Split\n",
            1,
        )
        .unwrap();
        assert_eq!(body, Body::default());
        assert_eq!(body.reproduction, None);
    }

    #[test]
    fn a_reproduction_section_is_recorded_even_when_empty() {
        let body = parse_body("## Reproduction\n", 1).unwrap();
        assert_eq!(body.reproduction, Some(String::new()));
    }

    #[test]
    fn section_names_ignore_case() {
        let body = parse_body("## OPEN QUESTIONS\n- one?\n", 1).unwrap();
        assert_eq!(body.open_questions.len(), 1);
    }

    #[test]
    fn a_mistyped_section_is_refused_with_its_line() {
        assert_eq!(
            parse_body("## Intent\nx\n## Open question\n- hidden?\n", 10),
            Err(ParseError::UnknownSection {
                heading: "Open question".to_string(),
                line: 12,
            })
        );
    }

    #[test]
    fn a_repeated_section_is_refused() {
        assert_eq!(
            parse_body("## Intent\nx\n## intent\ny\n", 1),
            Err(ParseError::DuplicateSection {
                heading: "intent".to_string(),
                line: 3,
            })
        );
    }

    #[test]
    fn a_subheading_is_text_in_intent_and_refused_in_a_list_section() {
        let body = parse_body("## Intent\n### Background\nwords\n", 1).unwrap();
        assert_eq!(body.intent, "### Background\nwords");
        assert_eq!(
            parse_body("## Open questions\n### Later\n- q?\n", 1),
            Err(ParseError::UnknownSection {
                heading: "Later".to_string(),
                line: 2,
            })
        );
        assert_eq!(
            parse_body("## Invariants\n### Unconfirmd\n- q\n", 1),
            Err(ParseError::UnknownSection {
                heading: "Unconfirmd".to_string(),
                line: 2,
            })
        );
    }

    #[test]
    fn an_unclosed_code_fence_is_refused_because_it_would_swallow_later_sections() {
        assert_eq!(
            parse_body("## Intent\n```\ncode\n## Open questions\n- hidden?\n", 5),
            Err(ParseError::UnterminatedFence { line: 6 })
        );
    }

    #[test]
    fn prose_and_numbered_items_under_open_questions_still_count() {
        let body = parse_body("## Open questions\nWe never settled rounding.\n", 1).unwrap();
        assert_eq!(
            body.open_questions,
            vec!["We never settled rounding.".to_string()]
        );
        let body = parse_body("## Open questions\n1. first?\n2. second?\n", 1).unwrap();
        assert_eq!(body.open_questions.len(), 2);
    }

    #[test]
    fn a_criterion_heading_without_a_well_formed_id_keeps_its_words_as_text() {
        for heading in ["The total drops", "ac-1", "AC-01", "AC-", "AC-1a", "AC1"] {
            let body = parse_body(
                &format!("## Acceptance criteria\n### {heading}\nOracle: a test.\n"),
                1,
            )
            .unwrap();
            assert_eq!(body.criteria.len(), 1);
            assert_eq!(body.criteria[0].id, "", "heading {heading:?}");
            assert_eq!(body.criteria[0].text, heading);
        }
    }

    #[test]
    fn criteria_written_without_headings_become_one_criterion_with_no_id() {
        let body = parse_body("## Acceptance criteria\n\n- totals are right\n", 1).unwrap();
        assert_eq!(
            body.criteria,
            vec![Criterion {
                id: String::new(),
                text: "- totals are right".to_string(),
                oracle: String::new(),
            }]
        );
    }

    #[test]
    fn a_criterion_without_an_oracle_line_has_an_empty_oracle() {
        let body = parse_body("## Acceptance criteria\n### AC-1\nJust text.\n", 1).unwrap();
        assert_eq!(body.criteria[0].oracle, "");
        assert_eq!(body.criteria[0].text, "Just text.");
    }

    #[test]
    fn criterion_id_shape() {
        assert!(is_criterion_id("AC-1"));
        assert!(is_criterion_id("AC-27"));
        assert!(!is_criterion_id("AC-0"));
        assert!(!is_criterion_id("AC-01"));
        assert!(!is_criterion_id("ac-1"));
        assert!(!is_criterion_id("AC-"));
        assert!(!is_criterion_id("REQ-1"));
        assert!(!is_criterion_id("AC-1 "));
    }

    const SPLIT: &str = "\
## Split

Two children; the second waits for the first.

```yaml
children:
  - id: pricing
    criteria: [AC-1]
    layer: 0
    scope:
      touched_files: [src/cart.js]
      test_paths: [test/pricing/]
  - id: display
    criteria: [AC-2]
    layer: 1
    depends_on: [pricing]
    scope:
      create_paths: [src/banner/]
      test_paths: [test/banner/]
```
";

    #[test]
    fn reads_the_split_from_its_fenced_yaml_block() {
        let body = parse_body(SPLIT, 1).unwrap();
        assert_eq!(body.children.len(), 2);
        assert_eq!(body.children[0].id, "pricing");
        assert_eq!(body.children[0].layer, 0);
        assert_eq!(
            body.children[0].scope.touched_files,
            vec!["src/cart.js".to_string()]
        );
        assert_eq!(body.children[1].depends_on, vec!["pricing".to_string()]);
        assert_eq!(
            body.children[1].scope.create_paths,
            vec!["src/banner/".to_string()]
        );
    }

    #[test]
    fn the_split_reads_the_same_with_crlf() {
        let crlf = SPLIT.replace('\n', "\r\n");
        assert_eq!(parse_body(&crlf, 1).unwrap(), parse_body(SPLIT, 1).unwrap());
    }

    #[test]
    fn a_split_with_an_empty_list_or_an_empty_block_has_no_children() {
        let body = parse_body("## Split\n```yaml\nchildren: []\n```\n", 1).unwrap();
        assert!(body.children.is_empty());
        let body = parse_body("## Split\n```yaml\n```\n", 1).unwrap();
        assert!(body.children.is_empty());
    }

    #[test]
    fn a_split_written_as_prose_or_bad_yaml_is_refused() {
        for text in [
            "## Split\npricing first, then display\n",
            "## Split\n```yaml\nchildren: [\n```\n",
            "## Split\n```yaml\nchildren:\n  - id: a\n    layer: zero\n```\n",
            "## Split\n```yaml\nkids: []\n```\n",
            "## Split\n```yaml\nchildren: []\n```\n```yaml\nchildren: []\n```\n",
        ] {
            let err = parse_body(text, 1).unwrap_err();
            assert_eq!(err.code(), "split", "{text:?} gave {err:?}");
        }
    }
}
````

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0422]: cannot find struct, variant or union type `Criterion` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet. Until Task 5 calls it, a build without tests also warns that `parse_body` is never used. That is expected.

- [ ] **Step 3: Write the implementation of the body reader**

Add this to `crates/factory-spec/src/body.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/body.rs -->
````rust
//! The Markdown body of a spec: a fixed set of `## ` sections, read line by line.
//!
//! This is not a Markdown parser. The template (version 1) defines these sections, matched
//! without regard to case, in any order, each at most once:
//!
//! - `## Intent`: free text.
//! - `## Reproduction`: free text; bug fixes only.
//! - `## Invariants`: a list; then, optionally, `### Unconfirmed` and a second list.
//! - `## Acceptance criteria`: one `### AC-<n>` heading per criterion, its text, and a line
//!   starting `Oracle:`; everything from that line to the next heading is the oracle.
//! - `## Open questions`: a list.
//! - `## Split`: one fenced YAML block: `children:` and a list of children.
//!
//! An empty section means "none". Lines before the first `## ` heading are ignored. A `## `
//! line inside a fenced code block is text, not a heading.

use crate::types::{Child, Criterion, ParseError};
use serde::Deserialize;

/// The body sections of a spec, as parsed.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Body {
    pub intent: String,
    pub reproduction: Option<String>,
    pub invariants: Vec<String>,
    pub unconfirmed_invariants: Vec<String>,
    pub criteria: Vec<Criterion>,
    pub open_questions: Vec<String>,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Intent,
    Reproduction,
    Invariants,
    Unconfirmed,
    Criteria,
    OpenQuestions,
    Split,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SplitDoc {
    children: Vec<Child>,
}

/// True for `AC-1`, `AC-27`: `AC-`, then a decimal number with no leading zero.
pub(crate) fn is_criterion_id(s: &str) -> bool {
    match s.strip_prefix("AC-") {
        Some(n) => !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

/// Read list items. A line starting `- `, `* ` or `<number>. ` starts an item; any other
/// non-blank line continues the item before it, or, with no item before it, is an item itself,
/// so that prose under "Open questions" still counts as an open question.
fn list_items(lines: &[&str]) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() || t == "-" || t == "*" {
            continue;
        }
        let bullet = t.strip_prefix("- ").or_else(|| t.strip_prefix("* "));
        let numbered = t.split_once(". ").and_then(|(n, rest)| {
            (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then_some(rest)
        });
        match (bullet.or(numbered), items.last_mut()) {
            (Some(item), _) => items.push(item.trim().to_string()),
            (None, Some(last)) => {
                last.push(' ');
                last.push_str(t);
            }
            (None, None) => items.push(t.to_string()),
        }
    }
    items
}

fn text_of(lines: &[&str]) -> String {
    lines.join("\n").trim().to_string()
}

/// Split a criterion's lines at its `Oracle:` line.
fn criterion_from(heading: &str, lines: &[&str]) -> Criterion {
    let first = heading.split_whitespace().next().unwrap_or("");
    let candidate = first.trim_end_matches(':');
    let (id, title) = if is_criterion_id(candidate) {
        (candidate.to_string(), heading[first.len()..].trim())
    } else {
        (String::new(), heading.trim())
    };
    let oracle_at = lines.iter().position(|line| {
        line.trim_start_matches(|c: char| c == '*' || c == '_' || c.is_whitespace())
            .to_lowercase()
            .starts_with("oracle:")
    });
    let (text_lines, oracle) = match oracle_at {
        Some(i) => {
            let mut oracle_lines = lines[i..].to_vec();
            let first_line = oracle_lines[0];
            let after = first_line
                .split_once(':')
                .map(|(_, rest)| rest)
                .unwrap_or("");
            oracle_lines[0] = after.trim_start_matches(['*', '_']);
            (&lines[..i], text_of(&oracle_lines))
        }
        None => (lines, String::new()),
    };
    let mut text = String::from(title);
    let rest = text_of(text_lines);
    if !text.is_empty() && !rest.is_empty() {
        text.push('\n');
    }
    text.push_str(&rest);
    Criterion { id, text, oracle }
}

/// Read the split section: exactly one fenced block, holding YAML.
fn split_from(lines: &[&str]) -> Result<Vec<Child>, ParseError> {
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut open = false;
    let mut prose = false;
    for line in lines {
        if is_fence(line) {
            if !open {
                blocks.push(Vec::new());
            }
            open = !open;
        } else if open {
            if let Some(block) = blocks.last_mut() {
                block.push(line);
            }
        } else if !line.trim().is_empty() {
            prose = true;
        }
    }
    match blocks.len() {
        0 if prose => Err(ParseError::Split(
            "the section has text and no fenced YAML block".to_string(),
        )),
        0 => Ok(Vec::new()),
        1 => {
            let yaml = blocks[0].join("\n");
            if yaml.trim().is_empty() {
                return Ok(Vec::new());
            }
            serde_saphyr::from_str::<SplitDoc>(&yaml)
                .map(|doc| doc.children)
                .map_err(|e| ParseError::Split(e.to_string()))
        }
        n => Err(ParseError::Split(format!(
            "the section has {n} fenced blocks; it must have one"
        ))),
    }
}

/// Parse the body. `first_line` is the 1-based line number, in the file, of `text`'s first line.
pub(crate) fn parse_body(text: &str, first_line: usize) -> Result<Body, ParseError> {
    let mut seen: Vec<Section> = Vec::new();
    let mut current: Option<Section> = None;
    let mut in_fence = false;
    let mut fence_line = 0usize;

    let mut intent: Vec<&str> = Vec::new();
    let mut reproduction: Vec<&str> = Vec::new();
    let mut invariants: Vec<&str> = Vec::new();
    let mut unconfirmed: Vec<&str> = Vec::new();
    let mut open_questions: Vec<&str> = Vec::new();
    let mut split: Vec<&str> = Vec::new();
    // Each criterion: its heading (empty for text before the first heading) and its lines.
    let mut criteria: Vec<(&str, Vec<&str>)> = Vec::new();

    for (offset, line) in text.lines().enumerate() {
        let line_no = first_line + offset;
        if is_fence(line) {
            if !in_fence {
                fence_line = line_no;
            }
            in_fence = !in_fence;
        } else if !in_fence {
            if let Some(heading) = line.strip_prefix("## ") {
                let heading = heading.trim();
                let section = match heading.to_lowercase().as_str() {
                    "intent" => Section::Intent,
                    "reproduction" => Section::Reproduction,
                    "invariants" => Section::Invariants,
                    "acceptance criteria" => Section::Criteria,
                    "open questions" => Section::OpenQuestions,
                    "split" => Section::Split,
                    _ => {
                        return Err(ParseError::UnknownSection {
                            heading: heading.to_string(),
                            line: line_no,
                        })
                    }
                };
                if seen.contains(&section) {
                    return Err(ParseError::DuplicateSection {
                        heading: heading.to_string(),
                        line: line_no,
                    });
                }
                seen.push(section);
                current = Some(section);
                continue;
            }
            if let Some(heading) = line.strip_prefix("### ") {
                let heading = heading.trim();
                match current {
                    Some(Section::Criteria) => {
                        criteria.push((heading, Vec::new()));
                        continue;
                    }
                    Some(Section::Invariants) if heading.eq_ignore_ascii_case("unconfirmed") => {
                        current = Some(Section::Unconfirmed);
                        continue;
                    }
                    Some(Section::Intent) | Some(Section::Reproduction) | None => {}
                    Some(_) => {
                        return Err(ParseError::UnknownSection {
                            heading: heading.to_string(),
                            line: line_no,
                        })
                    }
                }
            }
        }
        match current {
            None => {}
            Some(Section::Intent) => intent.push(line),
            Some(Section::Reproduction) => reproduction.push(line),
            Some(Section::Invariants) => invariants.push(line),
            Some(Section::Unconfirmed) => unconfirmed.push(line),
            Some(Section::OpenQuestions) => open_questions.push(line),
            Some(Section::Split) => split.push(line),
            Some(Section::Criteria) => match criteria.last_mut() {
                Some((_, lines)) => lines.push(line),
                None if line.trim().is_empty() => {}
                None => criteria.push(("", vec![line])),
            },
        }
    }
    if in_fence {
        return Err(ParseError::UnterminatedFence { line: fence_line });
    }

    Ok(Body {
        intent: text_of(&intent),
        reproduction: seen
            .contains(&Section::Reproduction)
            .then(|| text_of(&reproduction)),
        invariants: list_items(&invariants),
        unconfirmed_invariants: list_items(&unconfirmed),
        criteria: criteria
            .iter()
            .map(|(heading, lines)| criterion_from(heading, lines))
            .collect(),
        open_questions: list_items(&open_questions),
        children: split_from(&split)?,
    })
}
````

Three choices here are deliberate and fail closed. A `## ` heading the template does not define is an error, because a mistyped "Open questions" would otherwise hide its questions. Prose under a list section counts as one item, for the same reason. A code fence that never closes is an error, because it would swallow every section after it.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 41 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-spec/src/lib.rs crates/factory-spec/src/body.rs
git commit -m "feat(factory-spec): read the Markdown body: sections, lists, criteria and the split"
```


### Task 5: `parse` and `sha256_hex`

**Files:**
- Create: `crates/factory-spec/src/parse.rs` (tests inline)
- Modify: `crates/factory-spec/src/lib.rs`

**Interfaces:**
- Consumes: `body::parse_body` (Task 4); `Kind`, `Tier`, `Spec`, `ParseError`, `TEMPLATE_VERSION` (Task 3); `Scope` (Task 2).
- Produces:
  - `pub fn parse(bytes: &[u8]) -> Result<Spec, ParseError>`
  - `pub fn sha256_hex(bytes: &[u8]) -> String` — 64 lower-case hex digits

- [ ] **Step 1: Write the failing tests for `parse` and the hash**

Make `crates/factory-spec/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-spec/src/lib.rs -->
```rust
//! `factory-spec`: how a factory spec file is laid out, how it is checked before signing, how
//! it divides into child units, and which files a unit may change.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in bytes or text
//! and get plain values back. The control plane and the harness both link this crate, so the
//! two cannot disagree about whether a spec is ready or whether two units collide.

mod body;
mod parse;
mod path;
mod scope;
mod types;

pub use parse::{parse, sha256_hex};
pub use path::PathFault;
pub use scope::Scope;
pub use types::{
    Child, ChildView, Criterion, FormsIndex, Kind, ParseError, Problem, Spec, Tier,
    TEMPLATE_VERSION,
};
```

Create `crates/factory-spec/src/parse.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/parse.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = "\
---
id: SPEC-0007
work_item: \"adbarc92/sandbox#12\"
kind: feature
tier: t2
touched_files:
  - src/cart.js
create_paths:
  - src/discount/
test_paths:
  - test/discount/
touched_tests: []
permitted_dependencies: [decimal.js]
expected_red: []
forms_affected: [pricing]
source_hash: \"9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"
template_version: \"1\"
---

# Percent discount codes

## Intent

A cart accepts one percent discount code.

## Acceptance criteria

### AC-1

A 10% code on a 100.00 cart totals 90.00.

Oracle: a unit test compares `total()` with the code applied.
";

    #[test]
    fn sha256_hex_matches_the_published_test_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_hash_covers_exact_bytes_so_line_endings_and_a_bom_change_it() {
        let lf = SPEC.as_bytes().to_vec();
        let crlf = SPEC.replace('\n', "\r\n").into_bytes();
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(&lf);
        let hashes = [sha256_hex(&lf), sha256_hex(&crlf), sha256_hex(&bom)];
        assert_ne!(hashes[0], hashes[1]);
        assert_ne!(hashes[0], hashes[2]);
        assert_ne!(hashes[1], hashes[2]);
        assert_eq!(hashes[0].len(), 64);
        assert!(hashes[0]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    }

    #[test]
    fn parses_the_header_fields() {
        let spec = parse(SPEC.as_bytes()).unwrap();
        assert_eq!(spec.id, "SPEC-0007");
        assert_eq!(spec.work_item, "adbarc92/sandbox#12");
        assert_eq!(spec.kind, Kind::Feature);
        assert_eq!(spec.tier, Tier::T2);
        assert_eq!(spec.scope.touched_files, vec!["src/cart.js".to_string()]);
        assert_eq!(spec.scope.create_paths, vec!["src/discount/".to_string()]);
        assert_eq!(spec.scope.test_paths, vec!["test/discount/".to_string()]);
        assert!(spec.scope.touched_tests.is_empty());
        assert_eq!(spec.permitted_dependencies, vec!["decimal.js".to_string()]);
        assert!(spec.expected_red.is_empty());
        assert_eq!(spec.forms_affected, vec!["pricing".to_string()]);
        assert_eq!(
            spec.source_hash,
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        );
        assert_eq!(spec.template_version, "1");
    }

    #[test]
    fn parses_the_body_after_the_header() {
        let spec = parse(SPEC.as_bytes()).unwrap();
        assert_eq!(spec.intent, "A cart accepts one percent discount code.");
        assert_eq!(spec.reproduction, None);
        assert_eq!(spec.criteria.len(), 1);
        assert_eq!(spec.criteria[0].id, "AC-1");
        assert!(spec.open_questions.is_empty());
        assert!(spec.children.is_empty());
    }

    #[test]
    fn crlf_and_a_byte_order_mark_parse_to_the_same_spec() {
        let lf = parse(SPEC.as_bytes()).unwrap();
        let crlf = SPEC.replace('\n', "\r\n");
        assert_eq!(parse(crlf.as_bytes()).unwrap(), lf);
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(crlf.as_bytes());
        assert_eq!(parse(&bom).unwrap(), lf);
    }

    #[test]
    fn omitted_lists_are_empty() {
        let text = "---\nid: S-1\nwork_item: w\nkind: chore\ntier: t1\nsource_hash: h\ntemplate_version: \"1\"\n---\n";
        let spec = parse(text.as_bytes()).unwrap();
        assert!(spec.scope.is_empty());
        assert!(spec.permitted_dependencies.is_empty());
        assert!(spec.expected_red.is_empty());
        assert!(spec.forms_affected.is_empty());
        assert_eq!(spec.intent, "");
    }

    #[test]
    fn not_utf8_is_refused() {
        assert_eq!(parse(&[0xFF, 0xFE, 0x00]), Err(ParseError::NotUtf8));
    }

    #[test]
    fn a_file_with_no_header_is_refused() {
        assert_eq!(parse(b""), Err(ParseError::MissingHeader));
        assert_eq!(parse(b"# Just Markdown\n"), Err(ParseError::MissingHeader));
        assert_eq!(
            parse(b"\n---\nid: x\n---\n"),
            Err(ParseError::MissingHeader)
        );
    }

    #[test]
    fn a_header_that_never_closes_is_refused() {
        assert_eq!(
            parse(b"---\nid: S-1\n## Intent\n"),
            Err(ParseError::UnterminatedHeader)
        );
    }

    #[test]
    fn header_faults_are_reported_as_header_errors() {
        let base = "id: S-1\nwork_item: w\nkind: chore\ntier: t1\nsource_hash: h\ntemplate_version: \"1\"\n";
        let cases = [
            // a missing required field
            base.replace("tier: t1\n", ""),
            // an unknown field: a typo must not silently empty a list
            format!("{base}touched_file: [src/a.rs]\n"),
            // a value outside the enumeration
            base.replace("kind: chore", "kind: refactor"),
            base.replace("tier: t1", "tier: t9"),
            // a list where a string belongs
            base.replace("work_item: w", "work_item: [w]"),
            // not YAML at all
            "id: [unclosed\n".to_string(),
            // an id that could leave its directory
            base.replace("id: S-1", "id: ../S-1"),
            base.replace("id: S-1", "id: \"\""),
        ];
        for header in cases {
            let text = format!("---\n{header}---\n");
            let err = parse(text.as_bytes()).unwrap_err();
            assert_eq!(err.code(), "header", "{header:?} gave {err:?}");
        }
    }

    #[test]
    fn a_repeated_header_key_is_refused_rather_than_resolved() {
        let text = "---
id: S-1
work_item: w
kind: chore
tier: t1
source_hash: h
template_version: \"1\"
tier: t3
---
";
        assert_eq!(parse(text.as_bytes()).unwrap_err().code(), "header");
    }

    #[test]
    fn unquoted_scalars_are_read_as_the_strings_they_were_written_as() {
        let text = "---
id: 0001
work_item: 12
kind: chore
tier: t1
source_hash: 00e1
template_version: 1
---
";
        let spec = parse(text.as_bytes()).unwrap();
        assert_eq!(spec.id, "0001");
        assert_eq!(spec.work_item, "12");
        assert_eq!(spec.source_hash, "00e1");
        assert_eq!(spec.template_version, "1");
    }

    #[test]
    fn an_empty_header_is_a_header_error() {
        assert_eq!(parse(b"---\n---\n").unwrap_err().code(), "header");
    }

    #[test]
    fn an_unknown_template_version_is_refused() {
        let text = "---\nid: S-1\nwork_item: w\nkind: chore\ntier: t1\nsource_hash: h\ntemplate_version: \"2\"\n---\n";
        assert_eq!(
            parse(text.as_bytes()),
            Err(ParseError::UnsupportedTemplate {
                found: "2".to_string()
            })
        );
    }

    #[test]
    fn body_errors_carry_file_line_numbers() {
        let text = "---\nid: S-1\nwork_item: w\nkind: chore\ntier: t1\nsource_hash: h\ntemplate_version: \"1\"\n---\n\n## Intnet\n";
        assert_eq!(
            parse(text.as_bytes()),
            Err(ParseError::UnknownSection {
                heading: "Intnet".to_string(),
                line: 10,
            })
        );
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved imports `parse::parse`, `parse::sha256_hex`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of `parse` and the hash**

Add this to `crates/factory-spec/src/parse.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/parse.rs -->
```rust
//! Reading a spec file: a YAML header between two `---` lines, then the Markdown body.
//!
//! A spec file does not contain its own signature. A signature is made over the SHA-256 of
//! the bytes exactly as they are ([`sha256_hex`]), so parsing never rewrites or normalises
//! them: a byte-order mark or a CRLF line ending is read through, and stays inside what the
//! hash covers.

use crate::body::parse_body;
use crate::scope::Scope;
use crate::types::{Kind, ParseError, Spec, Tier, TEMPLATE_VERSION};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    id: String,
    work_item: String,
    kind: Kind,
    tier: Tier,
    #[serde(default)]
    touched_files: Vec<String>,
    #[serde(default)]
    create_paths: Vec<String>,
    #[serde(default)]
    test_paths: Vec<String>,
    #[serde(default)]
    touched_tests: Vec<String>,
    #[serde(default)]
    permitted_dependencies: Vec<String>,
    #[serde(default)]
    expected_red: Vec<String>,
    #[serde(default)]
    forms_affected: Vec<String>,
    source_hash: String,
    template_version: String,
}

/// SHA-256 of `bytes`, as 64 lowercase hex digits. A spec's signature is taken over this
/// value, computed on the unmodified file.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A spec id names a file (`<id>.md`) and a branch, so it is letters, digits, `.`, `_` and `-`,
/// and does not start with a `.`.
fn is_spec_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Parse a spec file.
pub fn parse(bytes: &[u8]) -> Result<Spec, ParseError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ParseError::NotUtf8)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    let mut lines = text.split_inclusive('\n');
    match lines.next() {
        Some(first) if first.trim_end() == "---" => {}
        _ => return Err(ParseError::MissingHeader),
    }
    let mut header = String::new();
    let mut header_lines = 0usize;
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        header.push_str(line);
        header_lines += 1;
    }
    if !closed {
        return Err(ParseError::UnterminatedHeader);
    }
    let body_text: String = lines.collect();

    let h: Header =
        serde_saphyr::from_str(&header).map_err(|e| ParseError::Header(e.to_string()))?;
    if !is_spec_id(&h.id) {
        return Err(ParseError::Header(format!(
            "id {:?} must be letters, digits, '.', '_' or '-', and not start with '.'",
            h.id
        )));
    }
    if h.template_version != TEMPLATE_VERSION {
        return Err(ParseError::UnsupportedTemplate {
            found: h.template_version,
        });
    }
    // The opening `---`, the header's lines and the closing `---` come before the body.
    let body = parse_body(&body_text, header_lines + 3)?;

    Ok(Spec {
        id: h.id,
        work_item: h.work_item,
        kind: h.kind,
        tier: h.tier,
        scope: Scope {
            touched_files: h.touched_files,
            create_paths: h.create_paths,
            test_paths: h.test_paths,
            touched_tests: h.touched_tests,
        },
        permitted_dependencies: h.permitted_dependencies,
        expected_red: h.expected_red,
        forms_affected: h.forms_affected,
        source_hash: h.source_hash,
        template_version: h.template_version,
        intent: body.intent,
        reproduction: body.reproduction,
        invariants: body.invariants,
        unconfirmed_invariants: body.unconfirmed_invariants,
        criteria: body.criteria,
        open_questions: body.open_questions,
        children: body.children,
    })
}
```

A byte-order mark is stripped for reading only. `sha256_hex` is given the caller's bytes untouched, so the mark and the line endings stay inside what a signature covers. The id check exists because the id names a file and a branch: `../x` must not get that far.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 56 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-spec/src/lib.rs crates/factory-spec/src/parse.rs
git commit -m "feat(factory-spec): parse a spec file and hash its exact bytes"
```


### Task 6: The testkit's builders

**Files:**
- Create: `crates/factory-spec/src/testkit.rs` (tests inline)
- Modify: `crates/factory-spec/src/lib.rs`

**Interfaces:**
- Consumes: `parse` (Task 5); `Scope`, `Child`, `Spec`, `TEMPLATE_VERSION`.
- Produces, in `pub mod testkit`, compiled under `cfg(test)` or the `testkit` feature:
  - `pub fn child(id: &str, criteria: &[&str], layer: u32, depends_on: &[&str]) -> ChildBuilder`
  - `pub struct ChildBuilder` with `touched`, `create`, `tests`, `touched_tests` (each `fn(self, paths: &[&str]) -> Self`) and `fn build(self) -> Child`
  - `pub struct SpecBuilder` with `fn new() -> Self` (a spec that is ready to sign), the setters `id`, `kind`, `tier`, `template_version`, `reproduction`, `open_question`, `unconfirmed_invariant` (each `fn(self, &str) -> Self`), `touched_files`, `create_paths`, `test_paths`, `touched_tests`, `permitted_dependencies`, `expected_red`, `forms_affected`, `criteria` (each `fn(self, &[&str]) -> Self`), `fn child(self, child: ChildBuilder) -> Self`, `fn crlf(self) -> Self`, `fn bom(self) -> Self`, `fn build(&self) -> Vec<u8>` and `fn spec(&self) -> Spec`

The builders come before the split and validation tasks because those tasks' tests are written with them. A builder writes bytes and parses them back, so nothing a test builds bypasses the parser.

- [ ] **Step 1: Write the failing tests for the builders**

Make `crates/factory-spec/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-spec/src/lib.rs -->
```rust
//! `factory-spec`: how a factory spec file is laid out, how it is checked before signing, how
//! it divides into child units, and which files a unit may change.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in bytes or text
//! and get plain values back. The control plane and the harness both link this crate, so the
//! two cannot disagree about whether a spec is ready or whether two units collide.

mod body;
mod parse;
mod path;
mod scope;
mod types;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use parse::{parse, sha256_hex};
pub use path::PathFault;
pub use scope::Scope;
pub use types::{
    Child, ChildView, Criterion, FormsIndex, Kind, ParseError, Problem, Spec, Tier,
    TEMPLATE_VERSION,
};
```

Create `crates/factory-spec/src/testkit.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/testkit.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse, sha256_hex, Kind, Tier};

    #[test]
    fn the_default_builder_parses_to_what_it_was_given() {
        let spec = SpecBuilder::new().spec();
        assert_eq!(spec.id, "SPEC-0001");
        assert_eq!(spec.kind, Kind::Feature);
        assert_eq!(spec.tier, Tier::T1);
        assert_eq!(spec.scope.touched_files, vec!["src/cart.js".to_string()]);
        assert_eq!(spec.scope.test_paths, vec!["test/".to_string()]);
        assert_eq!(spec.criteria.len(), 1);
        assert_eq!(spec.criteria[0].id, "AC-1");
        assert!(!spec.criteria[0].oracle.is_empty());
        assert_eq!(spec.invariants.len(), 1);
        assert!(spec.unconfirmed_invariants.is_empty());
        assert!(spec.open_questions.is_empty());
        assert!(spec.children.is_empty());
        assert_eq!(spec.reproduction, None);
    }

    #[test]
    fn every_setter_reaches_the_parsed_spec() {
        let spec = SpecBuilder::new()
            .id("SPEC-0042")
            .kind("bugfix")
            .tier("t3")
            .touched_files(&["src/a.js", "src/b.js"])
            .create_paths(&["src/new/"])
            .test_paths(&["test/a/"])
            .touched_tests(&["test/old.test.js"])
            .permitted_dependencies(&["decimal.js"])
            .expected_red(&["test/gate.test.js > gate > ac1 holds"])
            .forms_affected(&["pricing"])
            .reproduction("Apply the code twice.")
            .criteria(&["AC-1", "AC-2"])
            .open_question("Do codes stack?")
            .unconfirmed_invariant("Codes ignore case.")
            .spec();
        assert_eq!(spec.id, "SPEC-0042");
        assert_eq!(spec.kind, Kind::Bugfix);
        assert_eq!(spec.tier, Tier::T3);
        assert_eq!(spec.scope.touched_files.len(), 2);
        assert_eq!(spec.scope.create_paths, vec!["src/new/".to_string()]);
        assert_eq!(
            spec.scope.touched_tests,
            vec!["test/old.test.js".to_string()]
        );
        assert_eq!(spec.permitted_dependencies, vec!["decimal.js".to_string()]);
        assert_eq!(
            spec.expected_red,
            vec!["test/gate.test.js > gate > ac1 holds".to_string()]
        );
        assert_eq!(spec.forms_affected, vec!["pricing".to_string()]);
        assert_eq!(spec.reproduction.as_deref(), Some("Apply the code twice."));
        assert_eq!(spec.criteria.len(), 2);
        assert_eq!(spec.open_questions, vec!["Do codes stack?".to_string()]);
        assert_eq!(
            spec.unconfirmed_invariants,
            vec!["Codes ignore case.".to_string()]
        );
    }

    #[test]
    fn children_round_trip_through_the_split_block() {
        let built = child("api", &["AC-2"], 1, &["core"])
            .touched(&["src/cart.js"])
            .create(&["src/api/"])
            .tests(&["test/api/"])
            .touched_tests(&["test/old.test.js"]);
        let expected = built.clone().build();
        let spec = SpecBuilder::new()
            .child(child("core", &["AC-1"], 0, &[]).tests(&["test/core/"]))
            .child(built)
            .spec();
        assert_eq!(spec.children.len(), 2);
        assert_eq!(spec.children[1], expected);
    }

    #[test]
    fn windows_separators_survive_the_yaml_quoting() {
        let spec = SpecBuilder::new().touched_files(&["src\\cart.js"]).spec();
        assert_eq!(spec.scope.touched_files, vec!["src\\cart.js".to_string()]);
    }

    #[test]
    fn crlf_and_bom_change_the_bytes_and_the_hash_but_not_the_spec() {
        let plain = SpecBuilder::new();
        let crlf = SpecBuilder::new().crlf();
        let both = SpecBuilder::new().crlf().bom();
        assert!(crlf.build().windows(2).any(|w| w == b"\r\n"));
        assert!(!plain.build().contains(&b'\r'));
        assert_eq!(&both.build()[..3], &[0xEF, 0xBB, 0xBF]);
        assert_ne!(sha256_hex(&plain.build()), sha256_hex(&crlf.build()));
        assert_ne!(sha256_hex(&crlf.build()), sha256_hex(&both.build()));
        assert_eq!(plain.spec(), crlf.spec());
        assert_eq!(plain.spec(), both.spec());
    }

    #[test]
    fn empty_lists_are_written_and_read_as_empty() {
        let spec = SpecBuilder::new()
            .touched_files(&[])
            .test_paths(&[])
            .criteria(&[])
            .spec();
        assert!(spec.scope.is_empty());
        assert!(spec.criteria.is_empty());
    }

    #[test]
    fn a_bad_kind_makes_bytes_that_do_not_parse() {
        let bytes = SpecBuilder::new().kind("refactor").build();
        assert_eq!(parse(&bytes).unwrap_err().code(), "header");
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0433]: failed to resolve: use of undeclared type `SpecBuilder`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the builders**

Add this to `crates/factory-spec/src/testkit.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/testkit.rs -->
````rust
//! Builders for this crate's types, and the shared test vectors.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature.
//! A builder writes spec **bytes** and parses them back, so everything a test builds has been
//! through the real parser.

use crate::scope::Scope;
use crate::types::{Child, Spec};

/// A YAML double-quoted scalar.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn flow_list(items: &[String]) -> String {
    let quoted_items: Vec<String> = items.iter().map(|s| quoted(s)).collect();
    format!("[{}]", quoted_items.join(", "))
}

fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Builds one child of a split. Start with [`child`].
#[derive(Debug, Clone)]
pub struct ChildBuilder {
    child: Child,
}

/// A child with the given id, criteria, layer and dependencies, and an empty scope.
pub fn child(id: &str, criteria: &[&str], layer: u32, depends_on: &[&str]) -> ChildBuilder {
    ChildBuilder {
        child: Child {
            id: id.to_string(),
            criteria: owned(criteria),
            scope: Scope::default(),
            layer,
            depends_on: owned(depends_on),
        },
    }
}

impl ChildBuilder {
    pub fn touched(mut self, paths: &[&str]) -> Self {
        self.child.scope.touched_files = owned(paths);
        self
    }

    pub fn create(mut self, paths: &[&str]) -> Self {
        self.child.scope.create_paths = owned(paths);
        self
    }

    pub fn tests(mut self, paths: &[&str]) -> Self {
        self.child.scope.test_paths = owned(paths);
        self
    }

    pub fn touched_tests(mut self, paths: &[&str]) -> Self {
        self.child.scope.touched_tests = owned(paths);
        self
    }

    pub fn build(self) -> Child {
        self.child
    }
}

/// Builds the bytes of a spec file. [`SpecBuilder::new`] is a spec that is ready to sign: one
/// criterion (`AC-1`) with an oracle, a one-file scope, a test directory, no open questions
/// and no split.
#[derive(Debug, Clone)]
pub struct SpecBuilder {
    id: String,
    work_item: String,
    kind: String,
    tier: String,
    scope: Scope,
    permitted_dependencies: Vec<String>,
    expected_red: Vec<String>,
    forms_affected: Vec<String>,
    template_version: String,
    intent: String,
    reproduction: Option<String>,
    invariants: Vec<String>,
    unconfirmed_invariants: Vec<String>,
    criteria: Vec<String>,
    open_questions: Vec<String>,
    children: Vec<Child>,
    crlf: bool,
    bom: bool,
}

impl Default for SpecBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SpecBuilder {
    pub fn new() -> Self {
        SpecBuilder {
            id: "SPEC-0001".to_string(),
            work_item: "example/sandbox#1".to_string(),
            kind: "feature".to_string(),
            tier: "t1".to_string(),
            scope: Scope {
                touched_files: vec!["src/cart.js".to_string()],
                create_paths: Vec::new(),
                test_paths: vec!["test/".to_string()],
                touched_tests: Vec::new(),
            },
            permitted_dependencies: Vec::new(),
            expected_red: Vec::new(),
            forms_affected: Vec::new(),
            template_version: crate::TEMPLATE_VERSION.to_string(),
            intent: "A cart accepts one percent discount code.".to_string(),
            reproduction: None,
            invariants: vec!["A total is never negative.".to_string()],
            unconfirmed_invariants: Vec::new(),
            criteria: vec!["AC-1".to_string()],
            open_questions: Vec::new(),
            children: Vec::new(),
            crlf: false,
            bom: false,
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    /// `feature`, `bugfix` or `chore`; anything else makes bytes that do not parse.
    pub fn kind(mut self, kind: &str) -> Self {
        self.kind = kind.to_string();
        self
    }

    /// `t1`, `t2` or `t3`; anything else makes bytes that do not parse.
    pub fn tier(mut self, tier: &str) -> Self {
        self.tier = tier.to_string();
        self
    }

    pub fn touched_files(mut self, paths: &[&str]) -> Self {
        self.scope.touched_files = owned(paths);
        self
    }

    pub fn create_paths(mut self, paths: &[&str]) -> Self {
        self.scope.create_paths = owned(paths);
        self
    }

    pub fn test_paths(mut self, paths: &[&str]) -> Self {
        self.scope.test_paths = owned(paths);
        self
    }

    pub fn touched_tests(mut self, paths: &[&str]) -> Self {
        self.scope.touched_tests = owned(paths);
        self
    }

    pub fn permitted_dependencies(mut self, names: &[&str]) -> Self {
        self.permitted_dependencies = owned(names);
        self
    }

    pub fn expected_red(mut self, ids: &[&str]) -> Self {
        self.expected_red = owned(ids);
        self
    }

    pub fn forms_affected(mut self, forms: &[&str]) -> Self {
        self.forms_affected = owned(forms);
        self
    }

    pub fn template_version(mut self, version: &str) -> Self {
        self.template_version = version.to_string();
        self
    }

    pub fn reproduction(mut self, text: &str) -> Self {
        self.reproduction = Some(text.to_string());
        self
    }

    /// Replace the criteria. Each id gets a line of text and an oracle.
    pub fn criteria(mut self, ids: &[&str]) -> Self {
        self.criteria = owned(ids);
        self
    }

    pub fn open_question(mut self, question: &str) -> Self {
        self.open_questions.push(question.to_string());
        self
    }

    pub fn unconfirmed_invariant(mut self, invariant: &str) -> Self {
        self.unconfirmed_invariants.push(invariant.to_string());
        self
    }

    pub fn child(mut self, child: ChildBuilder) -> Self {
        self.children.push(child.build());
        self
    }

    /// Write CRLF line endings.
    pub fn crlf(mut self) -> Self {
        self.crlf = true;
        self
    }

    /// Start the file with a UTF-8 byte-order mark.
    pub fn bom(mut self) -> Self {
        self.bom = true;
        self
    }

    /// The spec file's bytes.
    pub fn build(&self) -> Vec<u8> {
        let mut out = String::new();
        let mut line = |text: &str| {
            out.push_str(text);
            out.push('\n');
        };
        line("---");
        line(&format!("id: {}", quoted(&self.id)));
        line(&format!("work_item: {}", quoted(&self.work_item)));
        line(&format!("kind: {}", self.kind));
        line(&format!("tier: {}", self.tier));
        line(&format!(
            "touched_files: {}",
            flow_list(&self.scope.touched_files)
        ));
        line(&format!(
            "create_paths: {}",
            flow_list(&self.scope.create_paths)
        ));
        line(&format!(
            "test_paths: {}",
            flow_list(&self.scope.test_paths)
        ));
        line(&format!(
            "touched_tests: {}",
            flow_list(&self.scope.touched_tests)
        ));
        line(&format!(
            "permitted_dependencies: {}",
            flow_list(&self.permitted_dependencies)
        ));
        line(&format!("expected_red: {}", flow_list(&self.expected_red)));
        line(&format!(
            "forms_affected: {}",
            flow_list(&self.forms_affected)
        ));
        line(&format!("source_hash: {}", quoted(&"0".repeat(64))));
        line(&format!(
            "template_version: {}",
            quoted(&self.template_version)
        ));
        line("---");
        line("");
        line("## Intent");
        line("");
        line(&self.intent);
        line("");
        if let Some(reproduction) = &self.reproduction {
            line("## Reproduction");
            line("");
            line(reproduction);
            line("");
        }
        line("## Invariants");
        line("");
        for invariant in &self.invariants {
            line(&format!("- {invariant}"));
        }
        line("");
        if !self.unconfirmed_invariants.is_empty() {
            line("### Unconfirmed");
            line("");
            for invariant in &self.unconfirmed_invariants {
                line(&format!("- {invariant}"));
            }
            line("");
        }
        line("## Acceptance criteria");
        line("");
        for id in &self.criteria {
            line(&format!("### {id}"));
            line("");
            line(&format!("The behaviour {id} describes holds."));
            line("");
            line(&format!("Oracle: a unit test named for {id} checks it."));
            line("");
        }
        line("## Open questions");
        line("");
        for question in &self.open_questions {
            line(&format!("- {question}"));
        }
        if !self.children.is_empty() {
            line("");
            line("## Split");
            line("");
            line("```yaml");
            line("children:");
            for child in &self.children {
                line(&format!("  - id: {}", quoted(&child.id)));
                line(&format!("    criteria: {}", flow_list(&child.criteria)));
                line(&format!("    layer: {}", child.layer));
                line(&format!("    depends_on: {}", flow_list(&child.depends_on)));
                line("    scope:");
                line(&format!(
                    "      touched_files: {}",
                    flow_list(&child.scope.touched_files)
                ));
                line(&format!(
                    "      create_paths: {}",
                    flow_list(&child.scope.create_paths)
                ));
                line(&format!(
                    "      test_paths: {}",
                    flow_list(&child.scope.test_paths)
                ));
                line(&format!(
                    "      touched_tests: {}",
                    flow_list(&child.scope.touched_tests)
                ));
            }
            line("```");
        }
        if self.crlf {
            out = out.replace('\n', "\r\n");
        }
        let mut bytes = Vec::new();
        if self.bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        bytes.extend_from_slice(out.as_bytes());
        bytes
    }

    /// The built bytes, parsed. Panics if they do not parse: a builder used that way is a
    /// test bug.
    pub fn spec(&self) -> Spec {
        match crate::parse(&self.build()) {
            Ok(spec) => spec,
            Err(error) => panic!("SpecBuilder produced bytes that do not parse: {error}"),
        }
    }
}
````

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 63 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-spec/src/lib.rs crates/factory-spec/src/testkit.rs
git commit -m "feat(factory-spec): testkit builders that write spec bytes"
```


### Task 7: `validate_split` and `child_view`

**Files:**
- Create: `crates/factory-spec/src/split.rs` (tests inline)
- Modify: `crates/factory-spec/src/lib.rs`

**Interfaces:**
- Consumes: `Spec`, `ChildView`, `Problem` (Task 3); `Scope::{covers, overlaps, entries}` (Task 2); `path::check` (Task 1); the testkit (Task 6), in tests only.
- Produces:
  - `pub fn validate_split(spec: &Spec) -> Vec<Problem>` — empty when the split is valid **or** the spec has no children
  - `pub fn child_view(spec: &Spec, child_id: &str) -> Result<ChildView, Problem>` — `Err(Problem::UnknownChild)` or `Err(Problem::UnknownCriterion)`

- [ ] **Step 1: Write the failing tests for the split rules**

Make `crates/factory-spec/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-spec/src/lib.rs -->
```rust
//! `factory-spec`: how a factory spec file is laid out, how it is checked before signing, how
//! it divides into child units, and which files a unit may change.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in bytes or text
//! and get plain values back. The control plane and the harness both link this crate, so the
//! two cannot disagree about whether a spec is ready or whether two units collide.

mod body;
mod parse;
mod path;
mod scope;
mod split;
mod types;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use parse::{parse, sha256_hex};
pub use path::PathFault;
pub use scope::Scope;
pub use split::{child_view, validate_split};
pub use types::{
    Child, ChildView, Criterion, FormsIndex, Kind, ParseError, Problem, Spec, Tier,
    TEMPLATE_VERSION,
};
```

Create `crates/factory-spec/src/split.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/split.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{child, SpecBuilder};
    use crate::types::Criterion;

    /// A parent with AC-1..AC-3 and a valid split: `core` in layer 0, then `api` and `ui`
    /// side by side in layer 1.
    fn set() -> SpecBuilder {
        SpecBuilder::new()
            .criteria(&["AC-1", "AC-2", "AC-3"])
            .touched_files(&["src/cart.js"])
            .create_paths(&["src/api/", "src/ui/"])
            .test_paths(&["test/"])
            .child(
                child("core", &["AC-1"], 0, &[])
                    .touched(&["src/cart.js"])
                    .tests(&["test/core/"]),
            )
            .child(
                child("api", &["AC-2"], 1, &["core"])
                    .create(&["src/api/"])
                    .tests(&["test/api/"]),
            )
            .child(
                child("ui", &["AC-3"], 1, &["core"])
                    .create(&["src/ui/"])
                    .tests(&["test/ui/"]),
            )
    }

    fn codes(problems: &[Problem]) -> Vec<&'static str> {
        problems.iter().map(Problem::code).collect()
    }

    #[test]
    fn a_valid_split_has_no_problems() {
        assert_eq!(validate_split(&set().spec()), vec![]);
    }

    #[test]
    fn no_children_is_not_a_split_and_not_a_problem() {
        let spec = SpecBuilder::new().spec();
        assert!(spec.children.is_empty());
        assert_eq!(validate_split(&spec), vec![]);
    }

    #[test]
    fn one_child_is_not_a_split() {
        let spec = SpecBuilder::new()
            .child(child("only", &["AC-1"], 0, &[]).tests(&["test/"]))
            .spec();
        assert_eq!(validate_split(&spec), vec![Problem::SplitTooSmall]);
    }

    #[test]
    fn a_criterion_no_child_owns_is_reported() {
        let spec = set().criteria(&["AC-1", "AC-2", "AC-3", "AC-4"]).spec();
        assert_eq!(
            validate_split(&spec),
            vec![Problem::CriterionUnassigned {
                criterion: "AC-4".to_string()
            }]
        );
    }

    #[test]
    fn a_criterion_two_children_own_is_reported_with_both() {
        let mut spec = set().spec();
        spec.children[2].criteria.push("AC-2".to_string());
        assert_eq!(
            validate_split(&spec),
            vec![Problem::CriterionAssignedTwice {
                criterion: "AC-2".to_string(),
                children: vec!["api".to_string(), "ui".to_string()],
            }]
        );
    }

    #[test]
    fn a_child_naming_a_criterion_the_parent_lacks_is_reported() {
        let mut spec = set().spec();
        spec.children[0].criteria.push("AC-9".to_string());
        assert_eq!(
            validate_split(&spec),
            vec![Problem::UnknownCriterion {
                child: "core".to_string(),
                criterion: "AC-9".to_string(),
            }]
        );
    }

    #[test]
    fn a_child_scope_outside_the_parent_is_reported_with_the_path() {
        let mut spec = set().spec();
        spec.children[1].scope.create_paths = vec!["src/api/".to_string(), "src/tax/".to_string()];
        assert_eq!(
            validate_split(&spec),
            vec![Problem::ChildScopeOutsideParent {
                child: "api".to_string(),
                path: "src/tax/".to_string(),
            }]
        );
    }

    #[test]
    fn a_child_scope_that_climbs_out_with_dot_segments_is_outside() {
        let mut spec = set().spec();
        spec.children[1].scope.create_paths = vec!["src/api/../../secrets/".to_string()];
        // Text-wise the entry starts with the parent's `src/api/`, so only the path check
        // can catch it.
        assert_eq!(codes(&validate_split(&spec)), vec!["path_malformed"]);
    }

    #[test]
    fn a_child_without_test_paths_is_reported() {
        let mut spec = set().spec();
        spec.children[0].scope.test_paths.clear();
        assert_eq!(
            validate_split(&spec),
            vec![Problem::ChildTestPathsEmpty {
                child: "core".to_string()
            }]
        );
    }

    #[test]
    fn same_layer_children_that_overlap_are_reported_once() {
        let mut spec = set().spec();
        spec.children[2].scope.create_paths = vec!["SRC\\API\\widgets\\".to_string()];
        assert_eq!(
            validate_split(&spec),
            vec![Problem::ChildrenOverlap {
                layer: 1,
                a: "api".to_string(),
                b: "ui".to_string(),
            }]
        );
    }

    #[test]
    fn children_in_different_layers_may_share_files() {
        let mut spec = set().spec();
        spec.children[1].scope.touched_files = vec!["src/cart.js".to_string()];
        assert_eq!(validate_split(&spec), vec![]);
    }

    #[test]
    fn a_dependency_must_exist_and_be_in_a_strictly_earlier_layer() {
        let mut spec = set().spec();
        spec.children[1].depends_on = vec!["ui".to_string(), "ghost".to_string()];
        spec.children[0].depends_on = vec!["core".to_string()];
        assert_eq!(
            validate_split(&spec),
            vec![
                Problem::DependencyNotEarlier {
                    child: "core".to_string(),
                    dependency: "core".to_string(),
                },
                Problem::DependencyNotEarlier {
                    child: "api".to_string(),
                    dependency: "ui".to_string(),
                },
                Problem::UnknownDependency {
                    child: "api".to_string(),
                    dependency: "ghost".to_string(),
                },
            ]
        );
    }

    #[test]
    fn two_children_with_one_id_are_reported() {
        let mut spec = set().spec();
        spec.children[2].id = "api".to_string();
        assert!(codes(&validate_split(&spec)).contains(&"duplicate_child_id"));
    }

    #[test]
    fn child_view_returns_the_childs_criteria_in_full_and_its_scope() {
        let spec = set().spec();
        let view = child_view(&spec, "api").unwrap();
        assert_eq!(view.id, "api");
        assert_eq!(view.layer, 1);
        assert_eq!(view.depends_on, vec!["core".to_string()]);
        assert_eq!(view.scope, spec.children[1].scope);
        assert_eq!(
            view.criteria,
            vec![Criterion {
                id: "AC-2".to_string(),
                text: spec.criteria[1].text.clone(),
                oracle: spec.criteria[1].oracle.clone(),
            }]
        );
        assert!(!view.scope.contains("src/ui/button.js"));
        assert!(view.scope.contains("src/api/routes.js"));
    }

    #[test]
    fn child_view_refuses_an_unknown_child_and_an_unknown_criterion() {
        let mut spec = set().spec();
        assert_eq!(
            child_view(&spec, "ghost"),
            Err(Problem::UnknownChild {
                child: "ghost".to_string()
            })
        );
        spec.children[0].criteria = vec!["AC-9".to_string()];
        assert_eq!(
            child_view(&spec, "core"),
            Err(Problem::UnknownCriterion {
                child: "core".to_string(),
                criterion: "AC-9".to_string(),
            })
        );
    }

    #[test]
    fn a_spec_without_a_split_has_no_child_to_view() {
        let spec = SpecBuilder::new().spec();
        assert_eq!(
            child_view(&spec, "core").unwrap_err().code(),
            "unknown_child"
        );
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved imports `split::child_view`, `split::validate_split`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the split rules**

Add this to `crates/factory-spec/src/split.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/split.rs -->
```rust
//! The split of a spec into child units, and the mechanical rules that make one valid.
//!
//! A split passes when it has at least two children and four mechanical checks hold: the
//! children share out the parent's criteria with none left over and none held twice; no
//! child reaches outside the parent's scope; two children in one layer have disjoint scopes;
//! and every dependency points at a child in a lower-numbered layer. Whether the split is a
//! *sensible* one is not something these rules can see.

use crate::path;
use crate::types::{ChildView, Problem, Spec};
use std::collections::BTreeMap;

/// Every reason `spec`'s split is not valid. Empty when the split is valid, and also when the
/// spec has no children: it then runs as one unit.
pub fn validate_split(spec: &Spec) -> Vec<Problem> {
    let children = &spec.children;
    let mut problems = Vec::new();
    if children.is_empty() {
        return problems;
    }
    if children.len() == 1 {
        problems.push(Problem::SplitTooSmall);
    }

    let mut layer_of: BTreeMap<&str, u32> = BTreeMap::new();
    for child in children {
        if layer_of.insert(child.id.as_str(), child.layer).is_some() {
            problems.push(Problem::DuplicateChildId {
                id: child.id.clone(),
            });
        }
    }

    // Every parent criterion belongs to exactly one child.
    let mut owners: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for child in children {
        for criterion in &child.criteria {
            if spec.criteria.iter().any(|c| &c.id == criterion) {
                owners
                    .entry(criterion.as_str())
                    .or_default()
                    .push(child.id.clone());
            } else {
                problems.push(Problem::UnknownCriterion {
                    child: child.id.clone(),
                    criterion: criterion.clone(),
                });
            }
        }
    }
    for criterion in &spec.criteria {
        match owners.get(criterion.id.as_str()) {
            None => problems.push(Problem::CriterionUnassigned {
                criterion: criterion.id.clone(),
            }),
            Some(children) if children.len() > 1 => {
                problems.push(Problem::CriterionAssignedTwice {
                    criterion: criterion.id.clone(),
                    children: children.clone(),
                })
            }
            Some(_) => {}
        }
    }

    // Every child is a unit with its own tests, inside the parent's scope.
    for child in children {
        for entry in child.scope.entries() {
            if let Err(fault) = path::check(entry) {
                problems.push(Problem::PathMalformed {
                    path: entry.clone(),
                    fault,
                });
            }
        }
        if child.scope.test_paths.is_empty() {
            problems.push(Problem::ChildTestPathsEmpty {
                child: child.id.clone(),
            });
        }
        if let Err(path) = spec.scope.covers(&child.scope) {
            problems.push(Problem::ChildScopeOutsideParent {
                child: child.id.clone(),
                path,
            });
        }
    }

    // Children in the same layer do not overlap.
    for (i, a) in children.iter().enumerate() {
        for b in &children[i + 1..] {
            if a.layer == b.layer && a.scope.overlaps(&b.scope) {
                problems.push(Problem::ChildrenOverlap {
                    layer: a.layer,
                    a: a.id.clone(),
                    b: b.id.clone(),
                });
            }
        }
    }

    // A child depends only on children in strictly earlier layers.
    for child in children {
        for dependency in &child.depends_on {
            match layer_of.get(dependency.as_str()) {
                None => problems.push(Problem::UnknownDependency {
                    child: child.id.clone(),
                    dependency: dependency.clone(),
                }),
                Some(layer) if *layer >= child.layer => {
                    problems.push(Problem::DependencyNotEarlier {
                        child: child.id.clone(),
                        dependency: dependency.clone(),
                    })
                }
                Some(_) => {}
            }
        }
    }
    problems
}

/// The child `child_id` as a unit sees it: its criteria in full, and its scope, looked up in
/// the parent spec. Call it on a spec whose split is valid.
pub fn child_view(spec: &Spec, child_id: &str) -> Result<ChildView, Problem> {
    let child = spec
        .children
        .iter()
        .find(|c| c.id == child_id)
        .ok_or_else(|| Problem::UnknownChild {
            child: child_id.to_string(),
        })?;
    let mut criteria = Vec::new();
    for id in &child.criteria {
        let criterion = spec.criteria.iter().find(|c| &c.id == id).ok_or_else(|| {
            Problem::UnknownCriterion {
                child: child.id.clone(),
                criterion: id.clone(),
            }
        })?;
        criteria.push(criterion.clone());
    }
    Ok(ChildView {
        id: child.id.clone(),
        criteria,
        scope: child.scope.clone(),
        layer: child.layer,
        depends_on: child.depends_on.clone(),
    })
}
```

Children in *different* layers may share files: the later one starts from the earlier one's result. Only children in the same layer run side by side, so only they must be disjoint.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 79 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-spec/src/lib.rs crates/factory-spec/src/split.rs
git commit -m "feat(factory-spec): split validity and child_view"
```


### Task 8: `validate`

**Files:**
- Create: `crates/factory-spec/src/validate.rs` (tests inline)
- Modify: `crates/factory-spec/src/lib.rs`

**Interfaces:**
- Consumes: `validate_split` (Task 7); `FormsIndex`, `Problem`, `Spec` (Task 3); `path::{check, normalize, entries_overlap}` (Task 1).
- Produces: `pub fn validate(spec: &Spec, forms: &FormsIndex) -> Vec<Problem>` — empty means ready to sign. Order: open questions, unconfirmed invariants, scope, criteria, Forms, then everything `validate_split` returns.

- [ ] **Step 1: Write the failing tests for readiness**

Make `crates/factory-spec/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-spec/src/lib.rs -->
```rust
//! `factory-spec`: how a factory spec file is laid out, how it is checked before signing, how
//! it divides into child units, and which files a unit may change.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in bytes or text
//! and get plain values back. The control plane and the harness both link this crate, so the
//! two cannot disagree about whether a spec is ready or whether two units collide.

mod body;
mod parse;
mod path;
mod scope;
mod split;
mod types;
mod validate;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use parse::{parse, sha256_hex};
pub use path::PathFault;
pub use scope::Scope;
pub use split::{child_view, validate_split};
pub use types::{
    Child, ChildView, Criterion, FormsIndex, Kind, ParseError, Problem, Spec, Tier,
    TEMPLATE_VERSION,
};
pub use validate::validate;
```

Create `crates/factory-spec/src/validate.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/validate.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::PathFault;
    use crate::testkit::{child, SpecBuilder};

    fn no_forms() -> FormsIndex {
        FormsIndex::new()
    }

    #[test]
    fn the_builders_default_spec_is_ready() {
        assert_eq!(validate(&SpecBuilder::new().spec(), &no_forms()), vec![]);
    }

    #[test]
    fn open_questions_are_refused_with_their_count() {
        let spec = SpecBuilder::new()
            .open_question("Do codes stack?")
            .open_question("Minimum spend?")
            .spec();
        assert_eq!(
            validate(&spec, &no_forms()),
            vec![Problem::OpenQuestions { count: 2 }]
        );
    }

    #[test]
    fn unconfirmed_invariants_are_refused_with_their_count() {
        let spec = SpecBuilder::new()
            .unconfirmed_invariant("Codes are case-insensitive.")
            .spec();
        assert_eq!(
            validate(&spec, &no_forms()),
            vec![Problem::UnconfirmedInvariants { count: 1 }]
        );
    }

    #[test]
    fn an_empty_scope_is_refused_and_so_is_an_empty_test_paths() {
        let spec = SpecBuilder::new()
            .touched_files(&[])
            .create_paths(&[])
            .test_paths(&[])
            .spec();
        assert_eq!(
            validate(&spec, &no_forms()),
            vec![Problem::EmptyScope, Problem::EmptyTestPaths]
        );
        let spec = SpecBuilder::new().test_paths(&[]).spec();
        assert_eq!(validate(&spec, &no_forms()), vec![Problem::EmptyTestPaths]);
    }

    #[test]
    fn malformed_scope_entries_are_refused_one_by_one() {
        let spec = SpecBuilder::new()
            .touched_files(&["src/*.js", "/etc/hosts"])
            .create_paths(&["src/../../up/"])
            .spec();
        assert_eq!(
            validate(&spec, &no_forms()),
            vec![
                Problem::PathMalformed {
                    path: "src/*.js".to_string(),
                    fault: PathFault::Glob,
                },
                Problem::PathMalformed {
                    path: "/etc/hosts".to_string(),
                    fault: PathFault::Absolute,
                },
                Problem::PathMalformed {
                    path: "src/../../up/".to_string(),
                    fault: PathFault::DotSegment,
                },
            ]
        );
    }

    #[test]
    fn a_spec_with_no_criteria_is_refused() {
        let spec = SpecBuilder::new().criteria(&[]).spec();
        assert_eq!(validate(&spec, &no_forms()), vec![Problem::NoCriteria]);
    }

    #[test]
    fn a_criterion_without_an_id_or_without_an_oracle_is_refused() {
        let mut spec = SpecBuilder::new()
            .criteria(&["AC-1", "AC-2", "AC-3"])
            .spec();
        spec.criteria[1].id.clear();
        spec.criteria[2].oracle = "  ".to_string();
        assert_eq!(
            validate(&spec, &no_forms()),
            vec![
                Problem::CriterionWithoutId { position: 2 },
                Problem::CriterionWithoutOracle {
                    criterion: "AC-3".to_string()
                },
            ]
        );
        spec.criteria[1].oracle.clear();
        assert!(
            validate(&spec, &no_forms()).contains(&Problem::CriterionWithoutOracle {
                criterion: "#2".to_string()
            })
        );
    }

    #[test]
    fn a_repeated_criterion_id_is_refused_once() {
        let spec = SpecBuilder::new()
            .criteria(&["AC-1", "AC-2", "AC-1", "AC-1"])
            .spec();
        assert_eq!(
            validate(&spec, &no_forms()),
            vec![Problem::DuplicateCriterionId {
                id: "AC-1".to_string()
            }]
        );
    }

    #[test]
    fn a_forms_interface_file_needs_the_form_named() {
        let mut forms = FormsIndex::new();
        forms.insert("pricing", "src/cart.js");
        let spec = SpecBuilder::new().touched_files(&["src/cart.js"]).spec();
        assert_eq!(
            validate(&spec, &forms),
            vec![Problem::FormNotDeclared {
                form: "pricing".to_string(),
                path: "src/cart.js".to_string(),
            }]
        );
        let spec = SpecBuilder::new()
            .touched_files(&["src/cart.js"])
            .forms_affected(&["pricing"])
            .spec();
        assert_eq!(validate(&spec, &forms), vec![]);
    }

    #[test]
    fn a_directory_entry_that_holds_an_interface_file_needs_the_form_too() {
        let mut forms = FormsIndex::new();
        forms.insert("pricing", "src/pricing/api.js");
        forms.insert("ledger", "src/ledger/");
        let spec = SpecBuilder::new()
            .touched_files(&[])
            .create_paths(&["SRC\\Pricing\\", "src/ledger/entry.js"])
            .spec();
        assert_eq!(
            validate(&spec, &forms),
            vec![
                Problem::FormNotDeclared {
                    form: "ledger".to_string(),
                    path: "src/ledger/entry.js".to_string(),
                },
                Problem::FormNotDeclared {
                    form: "pricing".to_string(),
                    path: "SRC\\Pricing\\".to_string(),
                },
            ]
        );
    }

    #[test]
    fn an_empty_forms_index_and_an_unrelated_form_refuse_nothing() {
        let mut forms = FormsIndex::new();
        forms.insert("ledger", "src/ledger/api.js");
        assert_eq!(validate(&SpecBuilder::new().spec(), &forms), vec![]);
    }

    #[test]
    fn an_invalid_split_is_refused_by_validate_too() {
        let spec = SpecBuilder::new()
            .child(child("only", &["AC-1"], 0, &[]).tests(&["test/"]))
            .spec();
        assert_eq!(validate(&spec, &no_forms()), vec![Problem::SplitTooSmall]);
    }

    #[test]
    fn problems_come_in_a_stable_order() {
        let mut spec = SpecBuilder::new()
            .open_question("q?")
            .unconfirmed_invariant("i")
            .test_paths(&[])
            .spec();
        spec.criteria[0].oracle.clear();
        let codes: Vec<&str> = validate(&spec, &no_forms())
            .iter()
            .map(Problem::code)
            .collect();
        assert_eq!(
            codes,
            vec![
                "open_questions",
                "unconfirmed_invariants",
                "empty_test_paths",
                "criterion_without_oracle",
            ]
        );
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved import `validate::validate`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of readiness**

Add this to `crates/factory-spec/src/validate.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/validate.rs -->
```rust
//! Is a parsed spec ready to sign?
//!
//! [`validate`] returns every reason it is not. An empty list means ready. The control plane
//! runs this before it stores a signature, and the harness runs the same code before it
//! starts a unit, so the two cannot disagree.

use crate::path;
use crate::split::validate_split;
use crate::types::{FormsIndex, Problem, Spec};
use std::collections::BTreeSet;

/// Every reason `spec` is not ready to sign, in a stable order: open questions, unconfirmed
/// invariants, scope, criteria, Forms, then the split.
///
/// `forms` says which files are which Form's interface. A scope entry that reaches one needs
/// that Form named in the spec's `forms_affected`.
pub fn validate(spec: &Spec, forms: &FormsIndex) -> Vec<Problem> {
    let mut problems = Vec::new();

    if !spec.open_questions.is_empty() {
        problems.push(Problem::OpenQuestions {
            count: spec.open_questions.len(),
        });
    }
    if !spec.unconfirmed_invariants.is_empty() {
        problems.push(Problem::UnconfirmedInvariants {
            count: spec.unconfirmed_invariants.len(),
        });
    }

    if spec.scope.is_empty() {
        problems.push(Problem::EmptyScope);
    }
    if spec.scope.test_paths.is_empty() {
        problems.push(Problem::EmptyTestPaths);
    }
    for entry in spec.scope.entries() {
        if let Err(fault) = path::check(entry) {
            problems.push(Problem::PathMalformed {
                path: entry.clone(),
                fault,
            });
        }
    }

    if spec.criteria.is_empty() {
        problems.push(Problem::NoCriteria);
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut reported: BTreeSet<&str> = BTreeSet::new();
    for (index, criterion) in spec.criteria.iter().enumerate() {
        let position = index + 1;
        if criterion.id.is_empty() {
            problems.push(Problem::CriterionWithoutId { position });
        } else if !seen.insert(criterion.id.as_str()) && reported.insert(criterion.id.as_str()) {
            problems.push(Problem::DuplicateCriterionId {
                id: criterion.id.clone(),
            });
        }
        if criterion.oracle.trim().is_empty() {
            problems.push(Problem::CriterionWithoutOracle {
                criterion: if criterion.id.is_empty() {
                    format!("#{position}")
                } else {
                    criterion.id.clone()
                },
            });
        }
    }

    for (form, interface) in forms.entries() {
        if spec.forms_affected.iter().any(|named| named == form) {
            continue;
        }
        let interface = path::normalize(interface);
        for entry in spec.scope.entries() {
            if path::entries_overlap(&path::normalize(entry), &interface) {
                problems.push(Problem::FormNotDeclared {
                    form: form.to_string(),
                    path: entry.clone(),
                });
            }
        }
    }

    problems.extend(validate_split(spec));
    problems
}
```

The Form check uses the overlap rule, not equality: a scope entry `src/pricing/` reaches the interface file `src/pricing/api.js` just as surely as naming the file does.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 92 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-spec/src/lib.rs crates/factory-spec/src/validate.rs
git commit -m "feat(factory-spec): validate that a spec is ready to sign"
```


### Task 9: The shared vectors

**Files:**
- Create: `crates/factory-spec/vectors/.gitattributes`, `crates/factory-spec/vectors/index.yaml`, and eight files under `crates/factory-spec/vectors/specs/`
- Create: `crates/factory-spec/src/testkit/vectors.rs` (tests inline)
- Modify: `crates/factory-spec/src/testkit.rs` (one line)

**Interfaces:**
- Consumes: `parse`, `validate`, `child_view`, `sha256_hex`, `Scope`, `FormsIndex`, `Problem`.
- Produces, in `pub mod testkit::vectors`:
  - `pub const INDEX: &str` — `vectors/index.yaml`
  - `pub const SPEC_FILES: &[(&str, &str)]` — each file under `vectors/specs/`, by name
  - `pub fn run_all() -> Result<usize, Vec<String>>` — the number of cases checked, or one line per failed expectation

The vectors are data, not Rust, so that a second repository proves the same behaviour on the same bytes. That repository enables the feature and adds one test:

```rust
// in the consumer's Cargo.toml: factory-spec = { ..., features = ["testkit"] } under [dev-dependencies]
#[test]
fn factory_spec_vectors_pass() {
    assert_eq!(factory_spec::testkit::vectors::run_all(), Ok(33));
}
```

`vectors/.gitattributes` pins the files to LF on every checkout. Without it, git on Windows may check them out with CRLF; the runner refuses a vector file that contains a CR, so a wrong checkout fails with a message that says why.

- [ ] **Step 1: Write the vector data**

Ten files. Write each exactly as shown, with LF line endings and one newline at the end. Note the `'SRC\API\widgets\'` entry in `bad-split.md`: the single quotes keep the backslashes literal in YAML.

Create `crates/factory-spec/vectors/.gitattributes` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/.gitattributes -->
```text
* text eol=lf
```

Create `crates/factory-spec/vectors/index.yaml` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/index.yaml -->
```yaml
# Shared vectors for factory-spec. Every repository that links this crate runs them.
# Files are under vectors/specs/. Codes are Problem::code() and ParseError::code() values.

specs:
  - file: ready-feature.md
    parse: ok
    criteria: [AC-1, AC-2]
    problems: []
  - file: ready-set.md
    parse: ok
    criteria: [AC-1, AC-2, AC-3]
    children: [core, api, ui]
    problems: []
  - file: not-ready.md
    parse: ok
    criteria: [AC-1, ""]
    problems: [open_questions, unconfirmed_invariants, criterion_without_oracle, criterion_without_id]
  - file: bad-split.md
    parse: ok
    criteria: [AC-1, AC-2, AC-3]
    children: [api, ui]
    problems: [criterion_unassigned, child_scope_outside_parent, children_overlap, dependency_not_earlier]
  - file: form-interface.md
    parse: ok
    criteria: [AC-1]
    forms:
      pricing: [src/pricing/api.js]
    problems: [form_not_declared]
  - file: no-header.md
    parse: missing_header
  - file: unknown-section.md
    parse: unknown_section
  - file: unknown-field.md
    parse: header

# Scope overlap. `a` and `b` are scopes given as flat path lists.
overlaps:
  - { a: [src/cart.js], b: [src/cart.js], expect: true }
  - { a: [src/cart.js], b: [SRC/Cart.JS], expect: true }
  - { a: ['src\cart.js'], b: [src/cart.js], expect: true }
  - { a: [src/], b: [src/deep/file.rs], expect: true }
  - { a: [src/deep/file.rs], b: [src/], expect: true }
  - { a: [src/a/], b: [src/a/b/], expect: true }
  - { a: [src/a], b: [src/a/], expect: true }
  - { a: [src/a/], b: [src/ab/], expect: false }
  - { a: [src/a.rs], b: [src/a.rs.bak], expect: false }
  - { a: [src/a.rs, docs/], b: [test/, docs/guide.md], expect: true }
  - { a: [], b: [src/], expect: false }
  - { a: [], b: [], expect: false }

# Scope containment: is a change to the file `path` inside the scope?
contains:
  - { scope: [src/cart.js], path: src/cart.js, expect: true }
  - { scope: [src/cart.js], path: 'SRC\Cart.js', expect: true }
  - { scope: [src/], path: src/deep/file.rs, expect: true }
  - { scope: [src/], path: srcx/file.rs, expect: false }
  - { scope: [src/a/], path: src/a, expect: false }
  - { scope: [src/], path: src/../secrets.env, expect: false }
  - { scope: [src/], path: /src/file.rs, expect: false }
  - { scope: [src/], path: 'C:\repo\src\file.rs', expect: false }
  - { scope: [], path: src/file.rs, expect: false }

# The signed hash: SHA-256 over exact bytes, lowercase hex. `text` is a YAML double-quoted
# string, so \n and \r below are real line-ending bytes.
hashes:
  - { text: "", sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 }
  - { text: "abc", sha256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad }
  - { text: "---\nid: S-1\n---\n", sha256: 09c80776d45f18d493af190ed9e42997c2ee0d9c60077a4cb7288ee331e43a47 }
  - { text: "---\r\nid: S-1\r\n---\r\n", sha256: 8a7dea89f1c6dd5455630c738a082682c124c09afec005fa2dff2b92f51e4826 }
```

Create `crates/factory-spec/vectors/specs/ready-feature.md` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/specs/ready-feature.md -->
```markdown
---
id: SPEC-0001
work_item: "example/sandbox#1"
kind: feature
tier: t1
touched_files:
  - src/cart.js
create_paths:
  - src/discount/
test_paths:
  - test/
touched_tests: []
permitted_dependencies: []
expected_red: []
forms_affected: []
source_hash: "0000000000000000000000000000000000000000000000000000000000000000"
template_version: "1"
---

# Percent discount codes

## Intent

A cart accepts one percent discount code and shows the reduced total.

## Invariants

- A total is never negative.
- Prices are whole numbers of cents.

## Acceptance criteria

### AC-1

A 10% code on a cart of 10000 cents gives a total of 9000 cents.

Oracle: a unit test applies the code and compares `total()` with 9000.

### AC-2: Unknown codes

An unknown code leaves the total unchanged and reports the code as unknown.

Oracle: a unit test applies `NOPE` and compares the total and the reported error.

## Open questions
```

Create `crates/factory-spec/vectors/specs/ready-set.md` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/specs/ready-set.md -->
````markdown
---
id: SPEC-0002
work_item: "example/sandbox#2"
kind: feature
tier: t2
touched_files:
  - src/cart.js
create_paths:
  - src/api/
  - src/ui/
test_paths:
  - test/
touched_tests: []
permitted_dependencies: []
expected_red: []
forms_affected: []
source_hash: "0000000000000000000000000000000000000000000000000000000000000000"
template_version: "1"
---

## Intent

Discount codes, built as three units: the pricing core, then the API and the banner side by side.

## Invariants

- A total is never negative.

## Acceptance criteria

### AC-1

The pricing core applies a percent code.

Oracle: a unit test compares `total()` before and after the code.

### AC-2

The API accepts a code and returns the new total.

Oracle: a unit test posts a code to the handler and reads the total from the reply.

### AC-3

The banner shows the code that was applied.

Oracle: a unit test renders the banner and reads the code from its text.

## Open questions

## Split

The core goes first because both other children call it.

```yaml
children:
  - id: core
    criteria: [AC-1]
    layer: 0
    scope:
      touched_files: [src/cart.js]
      test_paths: [test/core/]
  - id: api
    criteria: [AC-2]
    layer: 1
    depends_on: [core]
    scope:
      create_paths: [src/api/]
      test_paths: [test/api/]
  - id: ui
    criteria: [AC-3]
    layer: 1
    depends_on: [core]
    scope:
      create_paths: [src/ui/]
      test_paths: [test/ui/]
```
````

Create `crates/factory-spec/vectors/specs/not-ready.md` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/specs/not-ready.md -->
```markdown
---
id: SPEC-0003
work_item: "example/sandbox#3"
kind: bugfix
tier: t1
touched_files:
  - src/cart.js
create_paths: []
test_paths:
  - test/
touched_tests: []
permitted_dependencies: []
expected_red: []
forms_affected: []
source_hash: "0000000000000000000000000000000000000000000000000000000000000000"
template_version: "1"
---

## Intent

Applying the same code twice must not reduce the total twice.

## Reproduction

Apply `SAVE10` to a cart of 10000 cents twice; the total reads 8100.

## Invariants

- A total is never negative.

### Unconfirmed

- Codes are compared without regard to case.

## Acceptance criteria

### AC-1

Applying a code a second time changes nothing.

### Second application is reported

The second application reports that the code is already applied.

Oracle: a unit test applies the code twice and reads the report.

## Open questions

- Should the second application be an error or a silent no-op?
- Does removing a code restore the original total?
```

Create `crates/factory-spec/vectors/specs/bad-split.md` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/specs/bad-split.md -->
````markdown
---
id: SPEC-0004
work_item: "example/sandbox#4"
kind: feature
tier: t1
touched_files:
  - src/cart.js
create_paths:
  - src/api/
  - src/ui/
test_paths:
  - test/
touched_tests: []
permitted_dependencies: []
expected_red: []
forms_affected: []
source_hash: "0000000000000000000000000000000000000000000000000000000000000000"
template_version: "1"
---

## Intent

A split with four separate faults.

## Invariants

- A total is never negative.

## Acceptance criteria

### AC-1

First behaviour.

Oracle: a unit test checks the first behaviour.

### AC-2

Second behaviour.

Oracle: a unit test checks the second behaviour.

### AC-3

Third behaviour.

Oracle: a unit test checks the third behaviour.

## Open questions

## Split

```yaml
children:
  - id: api
    criteria: [AC-1]
    layer: 0
    depends_on: [ui]
    scope:
      create_paths: [src/api/, src/tax/]
      test_paths: [test/api/]
  - id: ui
    criteria: [AC-2]
    layer: 0
    scope:
      create_paths: [src/ui/, 'SRC\API\widgets\']
      test_paths: [test/ui/]
```
````

Create `crates/factory-spec/vectors/specs/form-interface.md` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/specs/form-interface.md -->
```markdown
---
id: SPEC-0005
work_item: "example/sandbox#5"
kind: feature
tier: t1
touched_files: []
create_paths:
  - src/pricing/
test_paths:
  - test/
touched_tests: []
permitted_dependencies: []
expected_red: []
forms_affected: []
source_hash: "0000000000000000000000000000000000000000000000000000000000000000"
template_version: "1"
---

## Intent

Rework the pricing module, including the file other modules import.

## Invariants

- A total is never negative.

## Acceptance criteria

### AC-1

The pricing module exposes `total()`.

Oracle: a unit test imports `total` from the module's interface file and calls it.

## Open questions
```

Create `crates/factory-spec/vectors/specs/no-header.md` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/specs/no-header.md -->
```markdown
# Percent discount codes

## Intent

This file has no header.
```

Create `crates/factory-spec/vectors/specs/unknown-section.md` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/specs/unknown-section.md -->
```markdown
---
id: SPEC-0006
work_item: "example/sandbox#6"
kind: feature
tier: t1
touched_files:
  - src/cart.js
create_paths: []
test_paths:
  - test/
touched_tests: []
permitted_dependencies: []
expected_red: []
forms_affected: []
source_hash: "0000000000000000000000000000000000000000000000000000000000000000"
template_version: "1"
---

## Intent

The section below is mistyped, which would hide an open question.

## Acceptance criteria

### AC-1

A behaviour.

Oracle: a unit test.

## Open question

- Is this ever answered?
```

Create `crates/factory-spec/vectors/specs/unknown-field.md` with exactly this content:

<!-- plan-op: create crates/factory-spec/vectors/specs/unknown-field.md -->
```markdown
---
id: SPEC-0007
work_item: "example/sandbox#7"
kind: feature
tier: t1
touched_files:
  - src/cart.js
create_paths: []
test_paths:
  - test/
touched_test: []
permitted_dependencies: []
expected_red: []
forms_affected: []
source_hash: "0000000000000000000000000000000000000000000000000000000000000000"
template_version: "1"
---

## Intent

The header mistypes a field name.
```

- [ ] **Step 2: Write the failing tests for the vector runner**

In `crates/factory-spec/src/testkit.rs`, add this line and the blank line after it, directly above `use crate::scope::Scope;`:

<!-- plan-op: before-line crates/factory-spec/src/testkit.rs :: use crate::scope::Scope; -->
```rust
pub mod vectors;

```

Create `crates/factory-spec/src/testkit/vectors.rs` holding only its test module:

<!-- plan-op: create crates/factory-spec/src/testkit/vectors.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shared_vector_passes() {
        // 8 specs, 12 overlap cases, 9 containment cases, 4 hashes. A vector that is dropped
        // from the index changes this number.
        assert_eq!(run_all(), Ok(33));
    }

    #[test]
    fn the_embedded_files_are_not_empty() {
        assert!(!INDEX.is_empty());
        for (file, text) in SPEC_FILES {
            assert!(!text.is_empty(), "{file} is empty");
        }
    }
}
```

- [ ] **Step 3: Run the tests and watch them fail**

Run: `cargo test -p factory-spec`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find value `INDEX` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 4: Write the implementation of the vector runner**

Add this to `crates/factory-spec/src/testkit/vectors.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-spec/src/testkit/vectors.rs -->
```rust
//! The shared test vectors: data files under `vectors/`, compiled into the crate, and the
//! function that runs them.
//!
//! Two repositories link this crate. Each runs [`run_all`] in a test, so both prove the same
//! behaviour on the same data. The files are embedded at compile time; nothing is read from
//! disk when the vectors run.

use crate::scope::Scope;
use crate::types::{FormsIndex, Problem};
use crate::{child_view, parse, sha256_hex, validate};
use serde::Deserialize;
use std::collections::BTreeMap;

/// `vectors/index.yaml`: which cases exist and what each expects.
pub const INDEX: &str = include_str!("../../vectors/index.yaml");

/// Every spec file under `vectors/specs/`, by file name.
pub const SPEC_FILES: &[(&str, &str)] = &[
    (
        "ready-feature.md",
        include_str!("../../vectors/specs/ready-feature.md"),
    ),
    (
        "ready-set.md",
        include_str!("../../vectors/specs/ready-set.md"),
    ),
    (
        "not-ready.md",
        include_str!("../../vectors/specs/not-ready.md"),
    ),
    (
        "bad-split.md",
        include_str!("../../vectors/specs/bad-split.md"),
    ),
    (
        "form-interface.md",
        include_str!("../../vectors/specs/form-interface.md"),
    ),
    (
        "no-header.md",
        include_str!("../../vectors/specs/no-header.md"),
    ),
    (
        "unknown-section.md",
        include_str!("../../vectors/specs/unknown-section.md"),
    ),
    (
        "unknown-field.md",
        include_str!("../../vectors/specs/unknown-field.md"),
    ),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    specs: Vec<SpecCase>,
    overlaps: Vec<OverlapCase>,
    contains: Vec<ContainsCase>,
    hashes: Vec<HashCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecCase {
    file: String,
    /// `ok`, or the expected `ParseError::code()`.
    parse: String,
    /// Expected criterion ids, in order.
    #[serde(default)]
    criteria: Vec<String>,
    /// Expected child ids, in order.
    #[serde(default)]
    children: Vec<String>,
    /// The Forms index to validate against: form name to interface paths.
    #[serde(default)]
    forms: BTreeMap<String, Vec<String>>,
    /// Expected `Problem::code()` values, in any order.
    #[serde(default)]
    problems: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlapCase {
    a: Vec<String>,
    b: Vec<String>,
    expect: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainsCase {
    scope: Vec<String>,
    path: String,
    expect: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HashCase {
    text: String,
    sha256: String,
}

fn scope_of(paths: &[String]) -> Scope {
    Scope {
        touched_files: paths.to_vec(),
        ..Scope::default()
    }
}

fn check_spec(case: &SpecCase, failures: &mut Vec<String>) {
    let name = &case.file;
    let Some((_, text)) = SPEC_FILES.iter().find(|(file, _)| file == name) else {
        failures.push(format!(
            "{name}: the index names a file that is not embedded"
        ));
        return;
    };
    if text.contains('\r') {
        failures.push(format!(
            "{name}: has CR line endings; vectors are LF (see vectors/.gitattributes)"
        ));
    }
    let spec = match parse(text.as_bytes()) {
        Ok(spec) => {
            if case.parse != "ok" {
                failures.push(format!("{name}: parsed, expected error {}", case.parse));
                return;
            }
            spec
        }
        Err(error) => {
            if case.parse != error.code() {
                failures.push(format!(
                    "{name}: parse gave {}, expected {}",
                    error.code(),
                    case.parse
                ));
            }
            return;
        }
    };
    let criteria: Vec<&str> = spec.criteria.iter().map(|c| c.id.as_str()).collect();
    if criteria != case.criteria {
        failures.push(format!(
            "{name}: criteria {criteria:?}, expected {:?}",
            case.criteria
        ));
    }
    let children: Vec<&str> = spec.children.iter().map(|c| c.id.as_str()).collect();
    if children != case.children {
        failures.push(format!(
            "{name}: children {children:?}, expected {:?}",
            case.children
        ));
    }
    let mut forms = FormsIndex::new();
    for (form, paths) in &case.forms {
        for path in paths {
            forms.insert(form, path);
        }
    }
    let mut got: Vec<&str> = validate(&spec, &forms).iter().map(Problem::code).collect();
    let mut want: Vec<&str> = case.problems.iter().map(String::as_str).collect();
    got.sort_unstable();
    want.sort_unstable();
    if got != want {
        failures.push(format!("{name}: problems {got:?}, expected {want:?}"));
    }
    if case.problems.is_empty() {
        for child in &case.children {
            if let Err(problem) = child_view(&spec, child) {
                failures.push(format!("{name}: child_view({child}) gave {problem}"));
            }
        }
    }
}

/// Run every vector. `Ok` carries the number of cases checked; `Err` carries one line per
/// failed expectation.
pub fn run_all() -> Result<usize, Vec<String>> {
    let index: Index = match serde_saphyr::from_str(INDEX) {
        Ok(index) => index,
        Err(error) => return Err(vec![format!("vectors/index.yaml does not read: {error}")]),
    };
    let mut failures = Vec::new();
    for case in &index.specs {
        check_spec(case, &mut failures);
    }
    for (file, _) in SPEC_FILES {
        if !index.specs.iter().any(|case| case.file == *file) {
            failures.push(format!("{file}: embedded, and missing from the index"));
        }
    }
    for case in &index.overlaps {
        let (a, b) = (scope_of(&case.a), scope_of(&case.b));
        if a.overlaps(&b) != case.expect || b.overlaps(&a) != case.expect {
            failures.push(format!(
                "overlaps {:?} / {:?}: expected {}",
                case.a, case.b, case.expect
            ));
        }
    }
    for case in &index.contains {
        if scope_of(&case.scope).contains(&case.path) != case.expect {
            failures.push(format!(
                "contains {:?} / {:?}: expected {}",
                case.scope, case.path, case.expect
            ));
        }
    }
    for case in &index.hashes {
        let got = sha256_hex(case.text.as_bytes());
        if got != case.sha256 {
            failures.push(format!("sha256 of {:?}: got {got}", case.text));
        }
    }
    if failures.is_empty() {
        Ok(index.specs.len() + index.overlaps.len() + index.contains.len() + index.hashes.len())
    } else {
        Err(failures)
    }
}
```

If `every_shared_vector_passes` fails, the message lists each failed expectation. Fix the code or the data file that is wrong; never change the number 33 to make it pass.

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p factory-spec`

Expected: PASS, ending with `test result: ok. 94 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add crates/factory-spec/vectors crates/factory-spec/src/testkit.rs crates/factory-spec/src/testkit/vectors.rs
git commit -m "test(factory-spec): shared vectors and the runner a second repository can call"
```


### Task 10: Verify the lane and open the pull request

**Files:**
- No file changes.

**Interfaces:**
- Consumes: everything above.
- Produces: one pull request against `factory/m0`.

- [ ] **Step 1: Run the tests, with and without the feature**

```bash
cargo test -p factory-spec
cargo test -p factory-spec --features testkit
```

Expected, both times: `test result: ok. 94 passed; 0 failed; 0 ignored`.

- [ ] **Step 2: Run the format and lint gates**

```bash
cargo fmt --all -- --check
cargo clippy -p factory-spec --all-targets -- -D warnings
cargo clippy -p factory-spec --all-targets --features testkit -- -D warnings
```

Expected: `cargo fmt` prints nothing and exits 0; each `cargo clippy` ends with a `Finished` line and no warning. If `cargo fmt` reports a difference in a file of this lane, run `cargo fmt -p factory-spec`, re-run the tests, and commit the result as `style(factory-spec): cargo fmt`.

- [ ] **Step 3: Check purity and ownership**

```bash
grep -rnE "std::(fs|net|process|env|thread)|^use tokio|\.await" crates/factory-spec/src; echo "grep exit: $?"
git diff --stat origin/factory/m0 -- . ":(exclude)crates/factory-spec"
git status --short
```

Expected: the `grep` prints only `grep exit: 1` (no match); the `git diff --stat` prints nothing (no file outside the lane changed, `Cargo.lock` included); `git status --short` prints nothing.

- [ ] **Step 4: Push and open the pull request**

```bash
git push -u origin feat/m0-factory-spec
gh pr create --base factory/m0 --head feat/m0-factory-spec \
  --title "feat(factory-spec): the spec format, validation, the split and the scope rules" \
  --body "$(cat <<'EOF'
## What changed

New pure crate `crates/factory-spec` (lane CC-SPEC of factory milestone 0):

- `parse` reads a spec file (YAML header, Markdown body); `sha256_hex` hashes its exact bytes.
- `validate` lists every reason a spec is not ready to sign; `validate_split` and `child_view` cover the split.
- `Scope` carries the scope and overlap rules (`with_grants`, `contains`, `overlaps`).
- A `testkit` feature with `SpecBuilder`, and shared vectors under `vectors/` with `testkit::vectors::run_all()`.

No file outside `crates/factory-spec/` is touched.

## How it was verified

- `cargo test -p factory-spec` and `cargo test -p factory-spec --features testkit`
- `cargo fmt --all -- --check`
- `cargo clippy -p factory-spec --all-targets -- -D warnings`, with and without `--features testkit`
- a grep of `src/` for file, network, process, environment, thread and async use: no match

## Deviations from the shared-interface draft, for the reviewer

D1 to D7 in the plan `docs/superpowers/plans/2026-10-04-factory-m0-spec-and-presets.md`.

## Issue

None: this is lane CC-SPEC of the factory program's milestone 0.
EOF
)"
```

Expected: `gh` prints the pull request's URL. A repository hook inspects `gh pr create`; if it blocks the command, say so in your report and leave it blocked. Leave the pull request open for the reviewer.

- [ ] **Step 5: Write the final report**

End with a report: the files you added, the pull request link, what each Verify command actually printed (including anything that failed), anything you need from the coordinator, the judgement calls you made and why, and anything other lanes should know. Repeat deviations D1 to D7 and open questions Q1 to Q3 and Q9 from this plan's self-review so the reviewer sees them without opening the plan.

```bash
git log --oneline origin/factory/m0..HEAD
```

Expected: nine commits, one per Task 1 to 9.


## Lane CC-PRESETS

> **THE REPORT AND SOURCE FIXTURES IN THIS LANE ENCODE AN ASSUMPTION, NOT A FACT.**
>
> Two things are not yet verified and are being checked by separate spikes: the exact class-name and test-name shapes that cargo-nextest, vitest and `node --test` write into JUnit XML, and whether test ids enumerated from source equal the ids those runners report (spike S6); and whether each stack's setup and checks run offline in a discarded copy of the tree (spike S7). Every JUnit report in this lane, in the unit tests and under `vectors/reports/`, was written by hand to the shapes listed below. The code is built so that the shapes live in three small functions (`cargo_id`, `node_id` and `cargo::locate`) and in data files. Task 25 replaces the hand-written files with the spike's captured ones.

**What is assumed, what was looked up, and what was observed.** "Looked up" means read from the tool's own documentation or source on 2026-10-04; it is still unverified until S6 runs the tools. "Observed" means run on the machine this plan was written on.

| # | Statement | Status |
|---|---|---|
| A1 | cargo-nextest writes one `<testsuite>` per test binary and one `<testcase>` per test; `classname` is the binary id and `name` is the test's path inside the binary (`discount::tests::ac1_x`) | **Assumed.** Looked up: nextest's JUnit documentation (`nexte.st`, "JUnit support") says every test binary forms one `<testsuite>` and every test one `<testcase>`, and shows a binary id of the form `package::target`. Not run |
| A2 | The binary id is the package name for unit tests, `<package>::<file>` for a file under `tests/`, and `<package>::bin/<name>` for a binary target | **Assumed.** Not looked up in detail, not run |
| A3 | A package's name equals the name of the directory that holds its `src/` or `tests/` | **Assumed**, and known to be only a convention: `enumerate_ids(path, source)` cannot see `Cargo.toml`. A file with tests and no such directory (a package at the repository root) is refused with `UnknownPackage`. Open question Q4 |
| A4 | The test-name half of a cargo id is the module path from the file's place under `src/` or `tests/`, the inline `mod` blocks, and the function name | **Observed for libtest names.** The enumerator in this plan was run over `crates/fleet-core`, `crates/harness-protocol` and `crates/harness-conformance` and gave exactly the 68 names `cargo test -- --list` prints for them. nextest's own names were not observed |
| A5 | Vitest's JUnit reporter writes `classname` as the test file's path relative to vitest's root, and `name` as the `describe` names and the test name joined by `" > "` (written `&gt;` in the XML); each file is wrapped in a `<testsuite>` named after the file | **Assumed.** Looked up: vitest's reporter guide (`vitest.dev/guide/reporters`) shows exactly this, and the reporter's source defaults `classname` to the file path relative to root with `" > "` as the ancestor separator; both are configurable. Not run. If vitest's root is not the repository root (`cockpit/ui` in this repository), the reported path is not the repository path. Open question Q6 |
| A6 | `node --test --test-reporter=junit` writes a `describe` block as a nested `<testsuite name=...>`, a test as `<testcase name=... classname="test">`, a failure as a `<failure>` child and a skipped or todo test as a `<skipped>` child | **Observed** on Node 22.17.1 |
| A7 | That reporter also writes a `file` attribute on each `<testcase>`, holding a path that is, or can be made, repository-relative | **Assumed, and false on the Node this plan was written with.** Observed: Node 22.17.1 writes no `file` attribute, so two files that each declare `cart > ac1 ...` produce two identical test cases (Task 21 pins that behaviour in a test: the reader refuses the report). Looked up: Node's reporter source gained the attribute in a commit dated 2025-10-08, and takes its value from the test event's file, which is probably an absolute path. Open question Q5 |
| A8 | A retried test appears once, with `flakyFailure`, `flakyError`, `rerunFailure` or `rerunError` children | **Assumed.** Looked up in the same nextest page. This plan counts such a test as not passed (D14) |

If spike S6 contradicts A1, A2, A5 or A6, Task 25 changes the mapping functions. If it contradicts A3 or A7, the fix needs a decision that is not this lane's to make: stop and report.

**Owns:** `crates/factory-presets/**` and nothing else.

**Reads:** `crates/fleet-core/src/*.rs` and `crates/harness-protocol/src/types.rs` for conventions; `crates/fleet-core/src/transition.rs` as a real in-file `#[cfg(test)] mod tests`; `cockpit/ui/src/lib/*.test.ts` as real vitest source. It links none of them.

**Worktree and branch:** worktree `D:\MajorProjects\.swarm-wt\m0-cc-presets`, branch `feat/m0-factory-presets`, cut from `origin/factory/m0`; the pull request goes against `factory/m0`.

```bash
git -C /d/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C /d/MajorProjects/INFRASTRUCTURE/command-center worktree add /d/MajorProjects/.swarm-wt/m0-cc-presets -b feat/m0-factory-presets origin/factory/m0
cd /d/MajorProjects/.swarm-wt/m0-cc-presets
```

Every command in this lane runs from `D:\MajorProjects\.swarm-wt\m0-cc-presets`, in Git Bash.

**Needs:** lane CC-COORD merged into `factory/m0`. Tasks 11 to 24 need nothing else. Task 25 needs the findings of spike S6 (and reads S7's for one question), which the owner reads before this lane's pull request is opened.

**Blocks:** the owner's approval of the contract crates' public types and the `contracts-v0.2.0` tag; the lane that writes `.reqdrive/config.toml` for the sandbox repositories; in milestone 1, the harness's oracle and controls crates and the control plane's verification crate.

**Verify:**

```bash
cargo test -p factory-presets
cargo test -p factory-presets --features testkit
cargo fmt --all -- --check
cargo clippy -p factory-presets --all-targets -- -D warnings
cargo clippy -p factory-presets --all-targets --features testkit -- -D warnings
```

**Files this lane creates.**

| Path | Responsibility |
|---|---|
| `crates/factory-presets/src/lib.rs` | Module wiring, re-exports, `PRESETS_VERSION` |
| `crates/factory-presets/src/types.rs` | The `Preset` trait, `TestCase`, `TestStatus`, the three error enums, `test_id` |
| `crates/factory-presets/src/junit.rs` | JUnit XML to raw test cases, with the size and depth caps |
| `crates/factory-presets/src/marker.rs` | The criterion marker in a test name |
| `crates/factory-presets/src/protected.rs` | Protected path patterns for both presets |
| `crates/factory-presets/src/rust_scan.rs` | A small scanner for Rust source |
| `crates/factory-presets/src/cargo.rs` | `cargo`: test ids and test regions from Rust source |
| `crates/factory-presets/src/js_scan.rs` | A small scanner for JavaScript and TypeScript source |
| `crates/factory-presets/src/node.rs` | `node`: test ids and test regions from JS/TS source |
| `crates/factory-presets/src/manifest.rs` | `Cargo.toml` and `package.json`: what was added, and nothing else changed |
| `crates/factory-presets/src/secrets.rs` | `SecretRule` and the fixed rule list |
| `crates/factory-presets/src/presets.rs` | `CargoPreset`, `NodePreset`, raw case to test id, `preset(name)` |
| `crates/factory-presets/src/testkit.rs` | `ReportBuilder`, `xml_escape`, `passed` |
| `crates/factory-presets/src/testkit/vectors.rs` | The embedded vectors, `run_all`, `run_secrets` |
| `crates/factory-presets/vectors/**` | The shared vector data |

**Known limits of this lane's static reading,** each one failing closed (an id that is never reported, or an error) unless it says otherwise:

- Rust: a test function generated by a macro is refused when its name is a macro variable; `rstest`, `test_case`, `test_matrix` and `parameterized` functions are refused. A `#[path = "..."]` attribute that moves a module is not followed. A package with both a library and a binary: tests in a module of the binary are given the library's class.
- Rust, **not** failing closed: a test module kept in its own file (`#[cfg(test)] mod tests;` with the tests in `tests.rs`) is test code that nothing inside that file says is test code, unless the file starts with `#![cfg(test)]`. `test_regions` returns the declaration line only. Open question Q8.
- JavaScript: `.jsx` and `.tsx` test files are refused. `each`, `for`, `skipIf` and `runIf` forms are refused. `t.test(...)` subtests inside a `node --test` test are not enumerated, and their parent is reported as a suite, so the parent's id is never reported.

### Task 11: The preset contract: trait, data and errors

**Files:**
- Create: `crates/factory-presets/src/types.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs` (replace the skeleton's content)

**Interfaces:**
- Consumes: nothing.
- Produces (all `pub`):
  - `const PRESETS_VERSION: &str = "0.1.0"` (in `lib.rs`)
  - `const ID_SEPARATOR: &str = " > "`; `fn test_id(class: &str, name: &str) -> String`
  - `enum TestStatus { Passed, Failed, Errored, Skipped }` (`snake_case`); `struct TestCase { id: String, status: TestStatus }`
  - `trait Preset: Send + Sync` with exactly these methods:
    - `fn name(&self) -> &'static str`
    - `fn read_report(&self, junit_xml: &str) -> Result<Vec<TestCase>, ReportError>`
    - `fn enumerate_ids(&self, path: &str, source: &str) -> Result<Vec<String>, EnumerateError>`
    - `fn criterion_of(&self, test_id: &str) -> Option<String>`
    - `fn is_protected(&self, path: &str) -> bool`
    - `fn test_regions(&self, path: &str, source: &str) -> Vec<std::ops::Range<usize>>`
    - `fn added_dependencies(&self, path: &str, before: &str, after: &str) -> Result<Vec<String>, ManifestError>`
  - `enum ReportError { Empty, TooLarge { bytes: usize }, TooDeep, Malformed(String), NotJUnit { root: String }, MissingAttribute { attribute: &'static str }, DuplicateId { id: String } }`
  - `enum EnumerateError { UnsupportedFile { path: String }, Unparseable { line: usize }, NotLiteral { line: usize }, Parametrised { line: usize, by: String }, UnknownPackage { path: String }, DuplicateId { id: String } }`
  - `enum Side { Before, After }`; `enum ManifestError { NotAManifest { path: String }, Malformed { side: Side, detail: String }, OtherChange { key: String }, DependencyRemoved { name: String }, DependencyChanged { name: String }, UnsupportedSource { name: String } }`
  - each error enum has `fn code(&self) -> &'static str`, `Display` and `std::error::Error`.

- [ ] **Step 1: Confirm the skeleton is what this plan assumes**

Run:

```bash
git status --short
git branch --show-current
grep -n -A5 "^\[dependencies\]" crates/factory-presets/Cargo.toml
grep -n -A2 "^\[features\]\|^\[dev-dependencies\]" crates/factory-presets/Cargo.toml
```

Expected: a clean tree on `feat/m0-factory-presets`; under `[dependencies]` the four lines `serde = { workspace = true }`, `serde_json = { workspace = true }`, `quick-xml = "0.42"` and `toml = { version = "1", default-features = false, features = ["std", "parse", "serde"] }`; `testkit = []` under `[features]`; `regex = "1"` under `[dev-dependencies]`.

If the file is missing or any of those lines differs, stop and report to the coordinator. Do not edit `Cargo.toml` to make it match. The `quick-xml` minor matters: Task 12's code is written against 0.42 and does not compile against 0.38.

- [ ] **Step 2: Write the failing tests for the contract**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod types;

pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/types.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/types.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_id_joins_class_and_name_with_the_separator() {
        assert_eq!(
            test_id("cart", "discount::tests::ac1_applies"),
            "cart > discount::tests::ac1_applies"
        );
        assert_eq!(ID_SEPARATOR, " > ");
    }

    #[test]
    fn statuses_are_snake_case_on_the_wire() {
        let case = TestCase {
            id: "cart > a".to_string(),
            status: TestStatus::Errored,
        };
        assert_eq!(
            serde_json::to_value(&case).unwrap(),
            serde_json::json!({"id": "cart > a", "status": "errored"})
        );
        let back: TestCase = serde_json::from_str(r#"{"id":"x","status":"skipped"}"#).unwrap();
        assert_eq!(back.status, TestStatus::Skipped);
    }

    #[test]
    fn error_codes_are_distinct_within_each_error_type() {
        fn distinct(mut codes: Vec<&str>) {
            let before = codes.len();
            codes.sort_unstable();
            codes.dedup();
            assert_eq!(codes.len(), before);
        }
        distinct(vec![
            ReportError::Empty.code(),
            ReportError::TooLarge { bytes: 1 }.code(),
            ReportError::TooDeep.code(),
            ReportError::Malformed("x".into()).code(),
            ReportError::NotJUnit { root: "x".into() }.code(),
            ReportError::MissingAttribute { attribute: "name" }.code(),
            ReportError::DuplicateId { id: "x".into() }.code(),
        ]);
        distinct(vec![
            EnumerateError::UnsupportedFile { path: "x".into() }.code(),
            EnumerateError::Unparseable { line: 1 }.code(),
            EnumerateError::NotLiteral { line: 1 }.code(),
            EnumerateError::Parametrised {
                line: 1,
                by: "x".into(),
            }
            .code(),
            EnumerateError::UnknownPackage { path: "x".into() }.code(),
            EnumerateError::DuplicateId { id: "x".into() }.code(),
        ]);
        distinct(vec![
            ManifestError::NotAManifest { path: "x".into() }.code(),
            ManifestError::Malformed {
                side: Side::Before,
                detail: "x".into(),
            }
            .code(),
            ManifestError::OtherChange { key: "x".into() }.code(),
            ManifestError::DependencyRemoved { name: "x".into() }.code(),
            ManifestError::DependencyChanged { name: "x".into() }.code(),
            ManifestError::UnsupportedSource { name: "x".into() }.code(),
        ]);
    }

    #[test]
    fn every_error_displays_something() {
        assert!(!ReportError::DuplicateId { id: "x".into() }
            .to_string()
            .is_empty());
        assert!(!EnumerateError::NotLiteral { line: 3 }
            .to_string()
            .is_empty());
        assert!(!ManifestError::OtherChange {
            key: "scripts".into()
        }
        .to_string()
        .is_empty());
    }
}
```

- [ ] **Step 3: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved imports `types::test_id`, `types::EnumerateError`, `types::ManifestError`, `types::Preset`, `types::ReportError`, `types::Side`, `types::TestCase`, `types::TestStatus`, `types::ID_SEPARATOR`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 4: Write the implementation of the contract**

Add this to `crates/factory-presets/src/types.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/types.rs -->
```rust
//! The preset contract: what a stack must answer, and the data those answers are made of.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Range;

/// What joins a report's class name and test name into one test id, and what joins the
/// parts of a nested test name. `src/cart.test.ts > cart > ac1 applies a code`.
pub const ID_SEPARATOR: &str = " > ";

/// The canonical test id for a class name and a test name.
pub fn test_id(class: &str, name: &str) -> String {
    format!("{class}{ID_SEPARATOR}{name}")
}

/// How one test ended, as its report says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    Passed,
    Failed,
    Errored,
    Skipped,
}

/// One test in a report. Only `Passed` counts as a pass.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub status: TestStatus,
}

/// The rules of one stack. Every method is pure: text in, data out.
pub trait Preset: Send + Sync {
    /// `cargo` or `node`.
    fn name(&self) -> &'static str;

    /// Every test in a JUnit XML report: its id (the report's class name and test name, joined
    /// by [`ID_SEPARATOR`]) and how it ended. A report in which one id occurs twice is an
    /// error, never two entries.
    fn read_report(&self, junit_xml: &str) -> Result<Vec<TestCase>, ReportError>;

    /// The ids of the tests declared in one source file, worked out from its text alone, in
    /// source order. `path` is repository-relative. If any test name in the file is not a
    /// literal this preset can read, the result is an error and never a shorter list. A file
    /// that is not test source gives no ids.
    fn enumerate_ids(&self, path: &str, source: &str) -> Result<Vec<String>, EnumerateError>;

    /// The criterion (`AC-1`) whose marker (`ac1`) appears in the name part of a test id.
    fn criterion_of(&self, test_id: &str) -> Option<String>;

    /// True if `path` configures the test runner, the build or an agent, and is therefore off
    /// limits to every unit.
    fn is_protected(&self, path: &str) -> bool;

    /// Where the test code inside a source file sits, as byte ranges, sorted and not
    /// overlapping.
    fn test_regions(&self, path: &str, source: &str) -> Vec<Range<usize>>;

    /// The names of the dependencies that `after` has and `before` lacks, sorted. An error if
    /// the two versions of the manifest differ in any other way.
    fn added_dependencies(
        &self,
        path: &str,
        before: &str,
        after: &str,
    ) -> Result<Vec<String>, ManifestError>;
}

/// A test report could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportError {
    /// The report is empty or only whitespace.
    Empty,
    /// The report is larger than [`crate::MAX_REPORT_BYTES`].
    TooLarge { bytes: usize },
    /// Elements are nested more deeply than [`crate::MAX_REPORT_DEPTH`].
    TooDeep,
    /// The report is not well-formed XML, or declares a document type. The text says which.
    Malformed(String),
    /// The root element is neither `testsuites` nor `testsuite`.
    NotJUnit { root: String },
    /// A `testcase` lacks an attribute the preset needs to form its id.
    MissingAttribute { attribute: &'static str },
    /// Two test cases have the same id.
    DuplicateId { id: String },
}

impl ReportError {
    /// A stable short name, used by the shared vectors.
    pub fn code(&self) -> &'static str {
        match self {
            ReportError::Empty => "empty",
            ReportError::TooLarge { .. } => "too_large",
            ReportError::TooDeep => "too_deep",
            ReportError::Malformed(_) => "malformed",
            ReportError::NotJUnit { .. } => "not_junit",
            ReportError::MissingAttribute { .. } => "missing_attribute",
            ReportError::DuplicateId { .. } => "duplicate_id",
        }
    }
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReportError::Empty => write!(f, "the report is empty"),
            ReportError::TooLarge { bytes } => write!(f, "the report is {bytes} bytes"),
            ReportError::TooDeep => write!(f, "the report's elements are nested too deeply"),
            ReportError::Malformed(detail) => write!(f, "the report is not XML: {detail}"),
            ReportError::NotJUnit { root } => {
                write!(
                    f,
                    "the report's root element is <{root}>, not a JUnit report"
                )
            }
            ReportError::MissingAttribute { attribute } => {
                write!(f, "a testcase has no {attribute} attribute")
            }
            ReportError::DuplicateId { id } => write!(f, "test id {id:?} appears twice"),
        }
    }
}

impl std::error::Error for ReportError {}

/// A source file's test ids could not be enumerated. The caller refuses to freeze the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumerateError {
    /// A test source file of a kind this preset cannot read (for `node`: `.jsx`, `.tsx`).
    UnsupportedFile { path: String },
    /// The file does not scan: an unterminated string or comment, or unbalanced brackets.
    Unparseable { line: usize },
    /// A test or suite name on this line is not a plain literal.
    NotLiteral { line: usize },
    /// A test on this line is generated from parameters, so one declaration is many tests.
    Parametrised { line: usize, by: String },
    /// The file declares tests and its path does not say which package they belong to.
    UnknownPackage { path: String },
    /// The file declares the same test id twice.
    DuplicateId { id: String },
}

impl EnumerateError {
    /// A stable short name, used by the shared vectors.
    pub fn code(&self) -> &'static str {
        match self {
            EnumerateError::UnsupportedFile { .. } => "unsupported_file",
            EnumerateError::Unparseable { .. } => "unparseable",
            EnumerateError::NotLiteral { .. } => "not_literal",
            EnumerateError::Parametrised { .. } => "parametrised",
            EnumerateError::UnknownPackage { .. } => "unknown_package",
            EnumerateError::DuplicateId { .. } => "duplicate_id",
        }
    }
}

impl fmt::Display for EnumerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnumerateError::UnsupportedFile { path } => {
                write!(f, "{path}: test ids cannot be read from this kind of file")
            }
            EnumerateError::Unparseable { line } => {
                write!(f, "line {line}: the file does not scan")
            }
            EnumerateError::NotLiteral { line } => {
                write!(f, "line {line}: the test name is not a plain literal")
            }
            EnumerateError::Parametrised { line, by } => {
                write!(f, "line {line}: the test is parametrised by {by}")
            }
            EnumerateError::UnknownPackage { path } => {
                write!(
                    f,
                    "{path}: the path does not name the package its tests belong to"
                )
            }
            EnumerateError::DuplicateId { id } => write!(f, "test id {id:?} is declared twice"),
        }
    }
}

impl std::error::Error for EnumerateError {}

/// Which of the two manifest versions a [`ManifestError::Malformed`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

/// A manifest change is not "dependencies were added, and nothing else".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The path is not this preset's manifest (`Cargo.toml`; `package.json`).
    NotAManifest { path: String },
    /// One version does not parse, or a dependency table is not a table.
    Malformed { side: Side, detail: String },
    /// Something outside the dependency tables differs. `key` is the first place it does.
    OtherChange { key: String },
    /// A dependency present before is gone.
    DependencyRemoved { name: String },
    /// A dependency present before has a different requirement.
    DependencyChanged { name: String },
    /// An added dependency does not come from the default registry by name: a path, a git
    /// URL, another registry, an alias or a tarball. A name check cannot vouch for it.
    UnsupportedSource { name: String },
}

impl ManifestError {
    /// A stable short name, used by the shared vectors.
    pub fn code(&self) -> &'static str {
        match self {
            ManifestError::NotAManifest { .. } => "not_a_manifest",
            ManifestError::Malformed { .. } => "malformed",
            ManifestError::OtherChange { .. } => "other_change",
            ManifestError::DependencyRemoved { .. } => "dependency_removed",
            ManifestError::DependencyChanged { .. } => "dependency_changed",
            ManifestError::UnsupportedSource { .. } => "unsupported_source",
        }
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::NotAManifest { path } => write!(f, "{path} is not a manifest"),
            ManifestError::Malformed { side, detail } => {
                write!(
                    f,
                    "the manifest {side:?} the change does not read: {detail}"
                )
            }
            ManifestError::OtherChange { key } => {
                write!(f, "the manifest changed outside its dependencies, at {key}")
            }
            ManifestError::DependencyRemoved { name } => {
                write!(f, "dependency {name} was removed")
            }
            ManifestError::DependencyChanged { name } => {
                write!(f, "dependency {name} was changed")
            }
            ManifestError::UnsupportedSource { name } => write!(
                f,
                "added dependency {name} does not come from the default registry by name"
            ),
        }
    }
}

impl std::error::Error for ManifestError {}
```

The trait is object-safe on purpose: callers hold `&'static dyn Preset`. `Send + Sync` lets that reference cross an `await`.

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/types.rs
git commit -m "feat(factory-presets): the Preset trait, test cases and the three error types"
```


### Task 12: Reading JUnit XML

**Files:**
- Create: `crates/factory-presets/src/junit.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: `ReportError`, `TestStatus` (Task 11); `quick_xml::{Reader, XmlVersion}`, `quick_xml::events::{BytesStart, Event}`.
- Produces:
  - `pub const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024`; `pub const MAX_REPORT_DEPTH: usize = 64`
  - `pub(crate) struct RawCase { suites: Vec<String>, classname: Option<String>, file: Option<String>, name: String, status: TestStatus }` — `suites` are the `name` attributes of the enclosing `<testsuite>` elements, outermost first
  - `pub(crate) fn read_cases(junit_xml: &str) -> Result<Vec<RawCase>, ReportError>`

This module knows XML and nothing about any runner: it does not form test ids. It treats the report as hostile, because the report is written inside a container that ran code the builder wrote.

- [ ] **Step 1: Write the failing tests for the JUnit reader**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod junit;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/junit.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/junit.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn one(xml: &str) -> RawCase {
        let mut cases = read_cases(xml).unwrap();
        assert_eq!(cases.len(), 1, "expected one case in {xml}");
        cases.remove(0)
    }

    #[test]
    fn reads_name_classname_file_and_enclosing_suites() {
        let case = one(
            r#"<testsuites><testsuite name="outer"><testsuite name="inner">
               <testcase name="works" classname="pkg" file="test/a.test.js"/>
               </testsuite></testsuite></testsuites>"#,
        );
        assert_eq!(
            case,
            RawCase {
                suites: vec!["outer".to_string(), "inner".to_string()],
                classname: Some("pkg".to_string()),
                file: Some("test/a.test.js".to_string()),
                name: "works".to_string(),
                status: TestStatus::Passed,
            }
        );
    }

    #[test]
    fn a_case_after_a_closed_suite_is_not_inside_it() {
        let cases = read_cases(
            r#"<testsuites>
               <testsuite name="a"><testcase name="in a"/></testsuite>
               <testsuite><testcase name="in unnamed"/></testsuite>
               <testsuite name="empty"/>
               <testcase name="top"/>
               </testsuites>"#,
        )
        .unwrap();
        let suites: Vec<(&str, Vec<String>)> = cases
            .iter()
            .map(|c| (c.name.as_str(), c.suites.clone()))
            .collect();
        assert_eq!(
            suites,
            vec![
                ("in a", vec!["a".to_string()]),
                ("in unnamed", vec![]),
                ("top", vec![]),
            ]
        );
    }

    #[test]
    fn a_bare_testsuite_root_is_a_report_too() {
        let case = one(r#"<testsuite name="s"><testcase name="t"/></testsuite>"#);
        assert_eq!(case.suites, vec!["s".to_string()]);
        assert_eq!(case.classname, None);
        assert_eq!(case.file, None);
    }

    fn status(inner: &str) -> TestStatus {
        one(&format!(
            r#"<testsuites><testcase name="t">{inner}</testcase></testsuites>"#
        ))
        .status
    }

    #[test]
    fn status_comes_from_the_child_elements() {
        assert_eq!(status(""), TestStatus::Passed);
        assert_eq!(status("<system-out>hello</system-out>"), TestStatus::Passed);
        assert_eq!(
            status(r#"<failure message="m">trace</failure>"#),
            TestStatus::Failed
        );
        assert_eq!(status(r#"<error type="E"/>"#), TestStatus::Errored);
        assert_eq!(status("<skipped/>"), TestStatus::Skipped);
    }

    #[test]
    fn a_retried_test_is_not_a_pass() {
        assert_eq!(status("<flakyFailure/>"), TestStatus::Failed);
        assert_eq!(status("<flakyError/>"), TestStatus::Errored);
        assert_eq!(status("<failure/><rerunFailure/>"), TestStatus::Failed);
        assert_eq!(status("<rerunError>x</rerunError>"), TestStatus::Errored);
    }

    #[test]
    fn the_worst_child_wins_and_only_direct_children_count() {
        assert_eq!(status("<skipped/><failure/><error/>"), TestStatus::Errored);
        assert_eq!(status("<error/><skipped/>"), TestStatus::Errored);
        assert_eq!(
            status("<system-out><failure/></system-out>"),
            TestStatus::Passed
        );
        assert_eq!(
            status("<system-out>&lt;failure/&gt; <![CDATA[<error/>]]></system-out>"),
            TestStatus::Passed
        );
    }

    #[test]
    fn xml_entities_in_names_are_decoded() {
        let case = one(r#"<testsuites><testsuite name="a &amp; b">
               <testcase name="cart &gt; ac1 &lt;10% &quot;off&quot; &#x2713; &#233;" classname="x&apos;y"/>
               </testsuite></testsuites>"#);
        assert_eq!(case.suites, vec!["a & b".to_string()]);
        assert_eq!(case.name, "cart > ac1 <10% \"off\" \u{2713} \u{e9}");
        assert_eq!(case.classname.as_deref(), Some("x'y"));
    }

    #[test]
    fn a_report_with_no_test_cases_is_an_empty_list() {
        assert_eq!(read_cases("<testsuites/>").unwrap(), vec![]);
        assert_eq!(
            read_cases(r#"<testsuites><testsuite name="s" tests="0"/></testsuites>"#).unwrap(),
            vec![]
        );
    }

    #[test]
    fn an_empty_or_blank_report_is_refused() {
        assert_eq!(read_cases(""), Err(ReportError::Empty));
        assert_eq!(read_cases(" \r\n\t"), Err(ReportError::Empty));
        assert_eq!(read_cases("\u{feff}"), Err(ReportError::Empty));
    }

    #[test]
    fn a_byte_order_mark_a_declaration_and_crlf_are_read_through() {
        let xml = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<testsuites>\r\n<testcase name=\"t\"/>\r\n</testsuites>\r\n";
        assert_eq!(one(xml).name, "t");
    }

    #[test]
    fn text_that_is_not_well_formed_xml_is_malformed() {
        for text in [
            "not xml",
            "{\"json\": true}",
            "<testsuites>",
            "<testsuites></wrong>",
            "<testsuites><testcase name=\"t\"></testsuites>",
            "<testsuites><testcase name=\"a\" name=\"b\"/></testsuites>",
            "<testsuites><testcase name=\"&bogus;\"/></testsuites>",
            "<testsuites/><testsuites/>",
            "</testsuites>",
        ] {
            let err = read_cases(text).unwrap_err();
            assert_eq!(err.code(), "malformed", "{text:?} gave {err:?}");
        }
    }

    #[test]
    fn a_document_type_declaration_is_refused_so_no_entity_can_expand() {
        let bomb = r#"<?xml version="1.0"?>
<!DOCTYPE lolz [
  <!ENTITY lol "lol">
  <!ENTITY lol2 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">
]>
<testsuites><testcase name="&lol2;"/></testsuites>"#;
        assert_eq!(read_cases(bomb).unwrap_err().code(), "malformed");
    }

    #[test]
    fn another_xml_document_is_not_a_junit_report() {
        assert_eq!(
            read_cases("<html><testcase name=\"t\"/></html>"),
            Err(ReportError::NotJUnit {
                root: "html".to_string()
            })
        );
    }

    #[test]
    fn a_testcase_without_a_name_is_refused() {
        assert_eq!(
            read_cases(r#"<testsuites><testcase classname="c"/></testsuites>"#),
            Err(ReportError::MissingAttribute { attribute: "name" })
        );
    }

    #[test]
    fn a_report_over_the_size_cap_is_refused_without_being_parsed() {
        let huge = " ".repeat(MAX_REPORT_BYTES + 1);
        assert_eq!(
            read_cases(&huge),
            Err(ReportError::TooLarge {
                bytes: MAX_REPORT_BYTES + 1
            })
        );
    }

    #[test]
    fn a_report_with_twenty_thousand_cases_reads_completely() {
        let mut xml = String::from("<testsuites><testsuite name=\"big\">");
        for n in 0..20_000 {
            xml.push_str(&format!(
                "<testcase name=\"t{n}\" classname=\"big\"><system-out>{}</system-out></testcase>",
                "x".repeat(200)
            ));
        }
        xml.push_str("</testsuite></testsuites>");
        let cases = read_cases(&xml).unwrap();
        assert_eq!(cases.len(), 20_000);
        assert_eq!(cases[19_999].name, "t19999");
        assert_eq!(cases[19_999].suites, vec!["big".to_string()]);
    }

    fn nested(depth: usize) -> String {
        let mut xml = String::new();
        for n in 0..depth {
            xml.push_str(&format!("<testsuite name=\"s{n}\">"));
        }
        xml.push_str("<testcase name=\"deep\"/>");
        for _ in 0..depth {
            xml.push_str("</testsuite>");
        }
        xml
    }

    #[test]
    fn nesting_up_to_the_cap_is_read_and_beyond_it_is_refused() {
        // The case is one element deeper than the innermost suite.
        let case = one(&nested(MAX_REPORT_DEPTH - 1));
        assert_eq!(case.suites.len(), MAX_REPORT_DEPTH - 1);
        assert_eq!(case.suites[0], "s0");
        assert_eq!(
            read_cases(&nested(MAX_REPORT_DEPTH)),
            Err(ReportError::TooDeep)
        );
    }

    #[test]
    fn a_hundred_thousand_nested_elements_are_refused_without_overflowing_the_stack() {
        assert_eq!(read_cases(&nested(100_000)), Err(ReportError::TooDeep));
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved imports `junit::MAX_REPORT_BYTES`, `junit::MAX_REPORT_DEPTH`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the JUnit reader**

Add this to `crates/factory-presets/src/junit.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/junit.rs -->
```rust
//! Reading a JUnit XML report into raw test cases. What makes a test id out of a raw case is
//! each preset's business; this module only knows the XML.
//!
//! A report is produced inside a container that ran code the builder wrote, so it is read as
//! hostile input. The XML is read as a stream of events with an explicit stack, so no call
//! recurses on the document's depth; the size and the nesting depth are capped; and a
//! document type declaration is refused, so no entity can be defined or expanded.

use crate::types::{ReportError, TestStatus};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

/// The largest report this crate reads: 64 MiB.
pub const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024;

/// The deepest element nesting this crate reads. Every test case carries the names of the
/// suites around it, so unbounded depth would let a small report ask for a great deal of
/// memory.
pub const MAX_REPORT_DEPTH: usize = 64;

/// One `<testcase>`, before a preset has formed its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawCase {
    /// `name` attributes of the enclosing `<testsuite>` elements, outermost first.
    pub suites: Vec<String>,
    pub classname: Option<String>,
    pub file: Option<String>,
    pub name: String,
    pub status: TestStatus,
}

/// An element that is open while the stream is read.
enum Open {
    /// A `<testsuite>`; `named` says whether it pushed a name onto the suite list.
    Suite {
        named: bool,
    },
    /// A `<testcase>`: the last entry of the case list.
    Case,
    Other,
}

fn rank(status: TestStatus) -> u8 {
    match status {
        TestStatus::Passed => 0,
        TestStatus::Skipped => 1,
        TestStatus::Failed => 2,
        TestStatus::Errored => 3,
    }
}

/// What a child element of a `<testcase>` says about it. A test that failed and was retried
/// is not a pass, whatever the retry did.
fn verdict(child: &str) -> Option<TestStatus> {
    match child {
        "failure" | "flakyFailure" | "rerunFailure" => Some(TestStatus::Failed),
        "error" | "flakyError" | "rerunError" => Some(TestStatus::Errored),
        "skipped" => Some(TestStatus::Skipped),
        _ => None,
    }
}

fn malformed(detail: impl ToString) -> ReportError {
    ReportError::Malformed(detail.to_string())
}

/// The value of the attribute `wanted` on `element`, with entities decoded and, as XML
/// requires of an attribute value, each tab or line break in it read as a space.
fn attribute(element: &BytesStart<'_>, wanted: &str) -> Result<Option<String>, ReportError> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(malformed)?;
        if attribute.key.as_ref() == wanted {
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(malformed)?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

/// Every `<testcase>` in the report, in document order.
pub(crate) fn read_cases(junit_xml: &str) -> Result<Vec<RawCase>, ReportError> {
    if junit_xml.len() > MAX_REPORT_BYTES {
        return Err(ReportError::TooLarge {
            bytes: junit_xml.len(),
        });
    }
    let text = junit_xml.strip_prefix('\u{feff}').unwrap_or(junit_xml);
    if text.trim().is_empty() {
        return Err(ReportError::Empty);
    }

    let mut reader = Reader::from_str(text);
    let mut cases: Vec<RawCase> = Vec::new();
    let mut suites: Vec<String> = Vec::new();
    let mut open: Vec<Open> = Vec::new();
    let mut seen_root = false;

    loop {
        let event = reader.read_event().map_err(malformed)?;
        let (element, stays_open) = match &event {
            Event::Start(element) => (element, true),
            Event::Empty(element) => (element, false),
            Event::End(_) => {
                match open.pop() {
                    Some(Open::Suite { named: true }) => {
                        suites.pop();
                    }
                    Some(_) => {}
                    None => return Err(malformed("a closing tag with no element open")),
                }
                continue;
            }
            Event::DocType(_) => {
                return Err(malformed("a document type declaration is not accepted"))
            }
            Event::Eof => break,
            _ => continue,
        };

        let tag = element.local_name().into_inner();
        if open.is_empty() {
            if seen_root {
                return Err(malformed("more than one root element"));
            }
            if tag != "testsuites" && tag != "testsuite" {
                return Err(ReportError::NotJUnit {
                    root: tag.to_string(),
                });
            }
            seen_root = true;
        }
        if open.len() >= MAX_REPORT_DEPTH {
            return Err(ReportError::TooDeep);
        }

        let opened = match tag {
            "testsuite" => {
                let name = attribute(element, "name")?;
                let named = name.is_some() && stays_open;
                if let (Some(name), true) = (name, stays_open) {
                    suites.push(name);
                }
                Open::Suite { named }
            }
            "testcase" => {
                let name = attribute(element, "name")?
                    .ok_or(ReportError::MissingAttribute { attribute: "name" })?;
                cases.push(RawCase {
                    suites: suites.clone(),
                    classname: attribute(element, "classname")?,
                    file: attribute(element, "file")?,
                    name,
                    status: TestStatus::Passed,
                });
                Open::Case
            }
            other => {
                if let (Some(Open::Case), Some(said), Some(case)) =
                    (open.last(), verdict(other), cases.last_mut())
                {
                    if rank(said) > rank(case.status) {
                        case.status = said;
                    }
                }
                Open::Other
            }
        };
        if stays_open {
            open.push(opened);
        }
    }

    if !open.is_empty() {
        return Err(malformed("the report ends inside an element"));
    }
    if !seen_root {
        return Err(malformed("the report has no root element"));
    }
    Ok(cases)
}
```

The reader keeps its own stack (`open`) and never recurses, so depth costs heap, not stack. The depth cap is there for a second reason: every case copies the names of the suites around it, so without a cap a report of a few megabytes could ask for terabytes.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 23 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/junit.rs
git commit -m "feat(factory-presets): read JUnit XML as a bounded event stream"
```


### Task 13: The criterion marker

**Files:**
- Create: `crates/factory-presets/src/marker.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: `ID_SEPARATOR` (Task 11).
- Produces: `pub(crate) fn criterion_of(test_id: &str) -> Option<String>` — `Some("AC-<n>")` for the first bounded `ac<n>` token in the id's name part.

- [ ] **Step 1: Write the failing tests for the marker**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod junit;
mod marker;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/marker.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/marker.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn of(id: &str) -> Option<String> {
        criterion_of(id)
    }

    #[test]
    fn finds_the_marker_in_rust_and_javascript_shaped_names() {
        assert_eq!(
            of("cart > discount::tests::ac1_applies_a_code"),
            Some("AC-1".into())
        );
        assert_eq!(
            of("cart > discount::tests::rejects_ac12"),
            Some("AC-12".into())
        );
        assert_eq!(
            of("src/cart.test.ts > cart > ac2 rejects unknown codes"),
            Some("AC-2".into())
        );
        assert_eq!(of("test/cart.test.js > [ac3] totals"), Some("AC-3".into()));
        assert_eq!(of("test/cart.test.js > totals (ac4)"), Some("AC-4".into()));
    }

    #[test]
    fn the_marker_is_bounded_on_both_sides() {
        assert_eq!(of("cart > tests::ac10_x"), Some("AC-10".into()));
        assert_ne!(of("cart > tests::ac10_x"), Some("AC-1".into()));
        assert_eq!(of("cart > tests::mac1_x"), None);
        assert_eq!(of("cart > tests::ac1x"), None);
        assert_eq!(of("cart > tests::ac1Rejects"), None);
        assert_eq!(of("cart > tests::xac1"), None);
        assert_eq!(of("cart > tests::\u{e9}ac1"), None);
    }

    #[test]
    fn only_the_lower_case_ac_form_with_a_number_is_a_marker() {
        assert_eq!(of("cart > tests::ac_applies"), None);
        assert_eq!(of("cart > tests::ac"), None);
        assert_eq!(of("cart > AC1 applies"), None);
        assert_eq!(of("cart > AC-1 applies"), None);
        assert_eq!(of("cart > tests::ac01_x"), None);
        assert_eq!(of("cart > tests::ac0"), None);
        assert_eq!(of("cart > tests::sha256_ac"), None);
    }

    #[test]
    fn the_first_marker_wins() {
        assert_eq!(of("cart > tests::ac2_then_ac1"), Some("AC-2".into()));
        assert_eq!(of("cart > tests::ac01_then_ac7"), Some("AC-7".into()));
    }

    #[test]
    fn the_class_part_is_not_searched() {
        assert_eq!(of("test/ac1/cart.test.js > totals"), None);
        assert_eq!(
            of("test/ac1/cart.test.js > ac2 totals"),
            Some("AC-2".into())
        );
    }

    #[test]
    fn an_id_without_a_separator_is_searched_whole() {
        assert_eq!(of("ac5_alone"), Some("AC-5".into()));
        assert_eq!(of(""), None);
        assert_eq!(of("a"), None);
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find function `criterion_of` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the marker**

Add this to `crates/factory-presets/src/marker.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/marker.rs -->
```rust
//! The criterion marker a test name carries.
//!
//! A test that checks criterion `AC-1` has the token `ac1` somewhere in its name:
//! `tests::ac1_applies_a_code`, `cart > ac1 applies a code`. The token is bounded: the
//! character before it and the character after it are not ASCII letters or digits, so `ac1`
//! does not match inside `ac10`, `mac1` or `ac1x`.

use crate::types::ID_SEPARATOR;

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte >= 0x80
}

/// The criterion id (`AC-<n>`) of the first marker in the test id's name. The class part of
/// the id (before the first separator) is not searched: a directory named `ac1/` marks nothing.
pub(crate) fn criterion_of(test_id: &str) -> Option<String> {
    let name = match test_id.split_once(ID_SEPARATOR) {
        Some((_class, name)) => name,
        None => test_id,
    };
    let bytes = name.as_bytes();
    let mut i = 0;
    while i + 2 <= bytes.len() {
        let at_boundary = i == 0 || !is_word(bytes[i - 1]);
        if at_boundary && bytes[i..].starts_with(b"ac") {
            let digits_start = i + 2;
            let mut end = digits_start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            let has_digits = end > digits_start && bytes[digits_start] != b'0';
            let bounded_after = end == bytes.len() || !is_word(bytes[end]);
            if has_digits && bounded_after {
                return Some(format!("AC-{}", &name[digits_start..end]));
            }
        }
        i += 1;
    }
    None
}
```

The marker is lower case only. That is what makes the boundary rule work for camel-case names: in `ac1Rejects` the `R` is a letter, so there is no marker, where an upper-case-tolerant rule would have to guess.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 29 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/marker.rs
git commit -m "feat(factory-presets): find the bounded criterion marker in a test name"
```


### Task 14: Protected path patterns

**Files:**
- Create: `crates/factory-presets/src/protected.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub(crate) fn cargo(path: &str) -> bool`; `pub(crate) fn node(path: &str) -> bool`.

- [ ] **Step 1: Write the failing tests for the patterns**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod junit;
mod marker;
mod protected;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/protected.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/protected.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_configuration_is_protected_on_both_stacks_anywhere_in_the_tree() {
        for path in [
            ".claude/settings.json",
            ".claude/agents/reviewer.md",
            "packages/web/.claude/commands/x.md",
            ".mcp.json",
            "tools/.mcp.json",
            "AGENTS.md",
            "docs/AGENTS.md",
            "CLAUDE.md",
            "crates/cart/CLAUDE.md",
        ] {
            assert!(cargo(path), "cargo should protect {path}");
            assert!(node(path), "node should protect {path}");
        }
    }

    #[test]
    fn matching_ignores_case_and_separator_style() {
        assert!(cargo("claude.md"));
        assert!(node("Docs\\Agents.MD"));
        assert!(cargo("crates\\cart\\.Claude\\settings.json"));
        assert!(cargo("crates\\cart\\BUILD.RS"));
        assert!(node("web\\Vitest.Config.TS"));
    }

    #[test]
    fn cargo_protects_build_scripts_and_toolchain_and_runner_configuration() {
        for path in [
            "build.rs",
            "crates/cart/build.rs",
            ".cargo/config.toml",
            "crates/cart/.cargo/config.toml",
            ".cargo/config",
            "rust-toolchain",
            "rust-toolchain.toml",
            "crates/cart/rust-toolchain.toml",
            ".config/nextest.toml",
        ] {
            assert!(cargo(path), "cargo should protect {path}");
        }
    }

    #[test]
    fn cargo_leaves_ordinary_files_alone() {
        for path in [
            "src/lib.rs",
            "src/build.rs.bak",
            "src/rebuild.rs",
            "crates/cart/Cargo.toml",
            "config.toml",
            "cargo/config.toml",
            "x.cargo/config.toml",
            "docs/claude.md.txt",
            "src/claude/mod.rs",
            "vitest.config.ts",
            "",
        ] {
            assert!(!cargo(path), "cargo should not protect {path:?}");
        }
    }

    #[test]
    fn node_protects_runner_and_bundler_configuration_and_npmrc() {
        for path in [
            "vitest.config.ts",
            "vitest.config.mts",
            "vitest.workspace.js",
            "cockpit/ui/vite.config.ts",
            "jest.config.js",
            "jest.config.json",
            "packages/a/jest.config.cjs",
            ".mocharc",
            ".mocharc.json",
            ".mocharc.yml",
            "packages/a/.mocharc.cjs",
            ".npmrc",
            "packages/a/.npmrc",
        ] {
            assert!(node(path), "node should protect {path}");
        }
    }

    #[test]
    fn node_leaves_ordinary_files_alone() {
        for path in [
            "src/cart.js",
            "test/cart.test.js",
            "package.json",
            "src/vitest.config.helper.ts",
            "vitest.config.ts.bak",
            "src/jest.config",
            "docs/npmrc.md",
            "build.rs",
            "",
        ] {
            assert!(!node(path), "node should not protect {path:?}");
        }
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find function `cargo` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the patterns**

Add this to `crates/factory-presets/src/protected.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/protected.rs -->
```rust
//! Protected path patterns: configuration that is off limits to every unit, wherever in the
//! tree it sits.
//!
//! These are patterns, not a list of files, because the files they protect may not exist yet:
//! a builder that adds a `vitest.config.ts` has changed how the tests run. Paths compare
//! case-insensitively, and `\` is read as `/`.

/// Lower case, `/` separators.
fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// True if `path` is `tail`, or ends with `/tail`.
fn ends_with_path(path: &str, tail: &str) -> bool {
    path == tail
        || path
            .strip_suffix(tail)
            .is_some_and(|head| head.ends_with('/'))
}

/// Agent configuration, protected on every stack: anything under a `.claude/` directory,
/// `.mcp.json`, `AGENTS.md` and `CLAUDE.md`.
fn agent_configuration(path: &str) -> bool {
    path.split('/').any(|segment| segment == ".claude")
        || matches!(basename(path), ".mcp.json" | "agents.md" | "claude.md")
}

/// The `cargo` preset's protected patterns.
pub(crate) fn cargo(path: &str) -> bool {
    let path = normalize(path);
    agent_configuration(&path)
        || matches!(
            basename(&path),
            "build.rs" | "rust-toolchain" | "rust-toolchain.toml"
        )
        || ends_with_path(&path, ".cargo/config.toml")
        || ends_with_path(&path, ".cargo/config")
        || ends_with_path(&path, ".config/nextest.toml")
}

/// The `node` preset's protected patterns.
pub(crate) fn node(path: &str) -> bool {
    let path = normalize(path);
    if agent_configuration(&path) {
        return true;
    }
    let file = basename(&path);
    if file == ".npmrc" || file == ".mocharc" || file.starts_with(".mocharc.") {
        return true;
    }
    const RUNNER_CONFIG: [&str; 4] = [
        "vitest.config.",
        "vitest.workspace.",
        "vite.config.",
        "jest.config.",
    ];
    const EXTENSIONS: [&str; 7] = ["js", "mjs", "cjs", "ts", "mts", "cts", "json"];
    RUNNER_CONFIG.iter().any(|stem| {
        file.strip_prefix(stem)
            .is_some_and(|extension| EXTENSIONS.contains(&extension))
    })
}
```

`build.rs` is matched by file name wherever it sits, so a module that happens to be called `src/build.rs` is protected too. That is the conservative side of a rule that cannot read `Cargo.toml`.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 35 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/protected.rs
git commit -m "feat(factory-presets): protected path patterns for cargo and node"
```


### Task 15: A scanner for Rust source

**Files:**
- Create: `crates/factory-presets/src/rust_scan.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces (all `pub(crate)`):
  - `enum Kind { Ident, Punct, Literal }`
  - `struct Tok<'a> { kind: Kind, text: &'a str, start: usize, end: usize, line: usize }` — `start..end` are byte offsets into the source; `line` is 1-based
  - `fn tokens(src: &str) -> Vec<Tok<'_>>` — comments and whitespace dropped; never fails
  - `fn matching(toks: &[Tok<'_>], open: usize) -> Option<usize>` — the index of the bracket that closes `toks[open]`

This crate may link an XML reader and a TOML/JSON reader and nothing else, so there is no Rust parser to lean on. The scanner does the one thing the next task needs: it makes comments, strings and character literals disappear, so that a `#[test]` inside a string is not mistaken for a test.

- [ ] **Step 1: Write the failing tests for the Rust scanner**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod junit;
mod marker;
mod protected;
mod rust_scan;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/rust_scan.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/rust_scan.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<&str> {
        tokens(src).iter().map(|t| t.text).collect()
    }

    #[test]
    fn splits_identifiers_punctuation_and_literals() {
        assert_eq!(
            texts("#[test]\nfn ac1_works() { assert_eq!(1, 1); }"),
            vec![
                "#",
                "[",
                "test",
                "]",
                "fn",
                "ac1_works",
                "(",
                ")",
                "{",
                "assert_eq",
                "!",
                "(",
                "1",
                ",",
                "1",
                ")",
                ";",
                "}"
            ]
        );
    }

    #[test]
    fn records_byte_offsets_and_lines() {
        let src = "mod a {\n    fn b() {}\n}\n";
        let toks = tokens(src);
        let b = toks.iter().find(|t| t.text == "b").unwrap();
        assert_eq!(b.line, 2);
        assert_eq!(&src[b.start..b.end], "b");
        let close = toks.last().unwrap();
        assert_eq!((close.text, close.line, close.end), ("}", 3, src.len() - 1));
    }

    #[test]
    fn comments_are_dropped_including_nested_block_comments() {
        assert_eq!(
            texts("a // #[test] fn x() {}\nb /* fn y() { /* nested */ } */ c"),
            vec!["a", "b", "c"]
        );
        let toks = tokens("/* one\ntwo */ z");
        assert_eq!(toks[0].line, 2);
    }

    #[test]
    fn strings_are_one_literal_whatever_they_hold() {
        let src =
            r##"let a = "fn x() { \" }"; let b = r#"mod "q" {"#; let c = b"}"; let d = c"{";"##;
        let toks = tokens(src);
        let literals: Vec<&str> = toks
            .iter()
            .filter(|t| t.kind == Kind::Literal)
            .map(|t| t.text)
            .collect();
        assert_eq!(
            literals,
            vec![
                r#""fn x() { \" }""#,
                r##"r#"mod "q" {"#"##,
                r#"b"}""#,
                r#"c"{""#
            ]
        );
        assert!(!toks
            .iter()
            .any(|t| t.kind == Kind::Punct && (t.text == "{" || t.text == "}")));
    }

    #[test]
    fn a_multi_line_string_advances_the_line_count() {
        let toks = tokens("let s = \"one\ntwo\";\nfn after() {}");
        let after = toks.iter().find(|t| t.text == "after").unwrap();
        assert_eq!(after.line, 3);
    }

    #[test]
    fn character_literals_are_literals_and_lifetimes_are_not() {
        let toks = tokens(r"let a = '{'; let b = '\''; let c = b'}'; fn f<'a>(x: &'a str) {}");
        let literals: Vec<&str> = toks
            .iter()
            .filter(|t| t.kind == Kind::Literal)
            .map(|t| t.text)
            .collect();
        assert_eq!(literals, vec!["'{'", r"'\''", "b'}'"]);
        let braces = toks
            .iter()
            .filter(|t| t.text == "{" || t.text == "}")
            .count();
        assert_eq!(braces, 2);
    }

    #[test]
    fn non_ascii_text_does_not_split_inside_a_character() {
        let src = "fn caf\u{e9}() { let s = \"\u{1F600}\"; let c = '\u{e9}'; }";
        let toks = tokens(src);
        assert!(toks.iter().any(|t| t.text == "caf\u{e9}"));
        assert!(toks.iter().any(|t| t.text == "'\u{e9}'"));
    }

    #[test]
    fn unterminated_text_ends_the_token_stream_without_panicking() {
        assert_eq!(texts("a \"never closed"), vec!["a", "\"never closed"]);
        assert_eq!(texts("a /* never closed"), vec!["a"]);
        assert_eq!(texts("a r#\"never closed"), vec!["a", "r#\"never closed"]);
        assert!(tokens("").is_empty());
    }

    #[test]
    fn matching_finds_the_closing_bracket_of_the_same_kind() {
        let toks = tokens("f(a[1], { (b) }) { x }");
        let open_paren = toks.iter().position(|t| t.text == "(").unwrap();
        let close = matching(&toks, open_paren).unwrap();
        assert_eq!(toks[close].text, ")");
        assert_eq!(toks[close + 1].text, "{");
        let last_open = toks.iter().rposition(|t| t.text == "{").unwrap();
        assert_eq!(matching(&toks, last_open), Some(toks.len() - 1));
        assert_eq!(matching(&toks, 0), None);
        let unbalanced = tokens("{ {");
        assert_eq!(matching(&unbalanced, 0), None);
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find function `tokens` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the Rust scanner**

Add this to `crates/factory-presets/src/rust_scan.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/rust_scan.rs -->
```rust
//! A small scanner for Rust source: enough to find attributes, `mod` and `fn` items and
//! matching braces, with comments, strings and character literals out of the way.
//!
//! It is not a Rust parser and does not try to be. It never fails: source it cannot make
//! sense of yields tokens that the callers' own checks then refuse.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// An identifier or keyword.
    Ident,
    /// One punctuation character.
    Punct,
    /// A string, character or number literal.
    Literal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tok<'a> {
    pub kind: Kind,
    pub text: &'a str,
    /// Byte offset of the token's first byte.
    pub start: usize,
    /// Byte offset one past the token's last byte.
    pub end: usize,
    /// 1-based line of the token's first byte.
    pub line: usize,
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

/// The index one past a `"`-delimited string whose opening quote is at `open`.
fn end_of_string(bytes: &[u8], open: usize) -> usize {
    let mut i = open + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// If a raw string (`r"..."`, `r#"..."#`, with an optional `b` or `c` before the `r`) starts at
/// `at`, the index one past it.
fn end_of_raw_string(bytes: &[u8], at: usize) -> Option<usize> {
    let mut i = at;
    if matches!(bytes.get(i), Some(b'b') | Some(b'c')) {
        i += 1;
    }
    if bytes.get(i) != Some(&b'r') {
        return None;
    }
    i += 1;
    let mut hashes = 0;
    while bytes.get(i) == Some(&b'#') {
        hashes += 1;
        i += 1;
    }
    if bytes.get(i) != Some(&b'"') {
        return None;
    }
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'"'
            && bytes[i + 1..]
                .iter()
                .take(hashes)
                .filter(|b| **b == b'#')
                .count()
                == hashes
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    Some(bytes.len())
}

/// If a character literal starts at the `'` at `at`, the index one past it. A lifetime
/// (`'a`) is not a character literal.
fn end_of_char(src: &str, at: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    if bytes.get(at + 1) == Some(&b'\\') {
        let mut i = at + 3;
        while i < bytes.len() && bytes[i] != b'\'' && bytes[i] != b'\n' {
            i += 1;
        }
        return (bytes.get(i) == Some(&b'\'')).then_some(i + 1);
    }
    let ch = src[at + 1..].chars().next()?;
    let after = at + 1 + ch.len_utf8();
    (ch != '\'' && bytes.get(after) == Some(&b'\'')).then_some(after + 1)
}

/// The tokens of `src`, in order, without comments or whitespace.
pub(crate) fn tokens(src: &str) -> Vec<Tok<'_>> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if byte.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            let mut depth = 0usize;
            while i < bytes.len() {
                if bytes[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if bytes[i] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
            continue;
        }

        let start = i;
        let prefixed_quote = matches!(byte, b'b' | b'c') && bytes.get(i + 1) == Some(&b'"');
        let kind = if let Some(end) = end_of_raw_string(bytes, i) {
            i = end;
            Kind::Literal
        } else if byte == b'"' || prefixed_quote {
            i = end_of_string(bytes, if byte == b'"' { i } else { i + 1 });
            Kind::Literal
        } else if byte == b'\'' || (byte == b'b' && bytes.get(i + 1) == Some(&b'\'')) {
            let quote = if byte == b'\'' { i } else { i + 1 };
            match end_of_char(src, quote) {
                Some(end) => {
                    i = end;
                    Kind::Literal
                }
                None if byte == b'\'' => {
                    i += 1;
                    Kind::Punct
                }
                None => {
                    i += 1;
                    Kind::Ident
                }
            }
        } else if is_ident_start(byte) {
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            Kind::Ident
        } else if byte.is_ascii_digit() {
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            Kind::Literal
        } else {
            i += 1;
            Kind::Punct
        };
        let end = i.min(bytes.len());
        i = end;
        out.push(Tok {
            kind,
            text: &src[start..end],
            start,
            end,
            line,
        });
        line += bytes[start..end].iter().filter(|b| **b == b'\n').count();
    }
    out
}

/// The index of the token that closes the bracket opened at `open` (`toks[open]` is `(`, `[`
/// or `{`), counting only that kind of bracket.
pub(crate) fn matching(toks: &[Tok<'_>], open: usize) -> Option<usize> {
    let opener = toks.get(open)?.text;
    let closer = match opener {
        "(" => ")",
        "[" => "]",
        "{" => "}",
        _ => return None,
    };
    let mut depth = 0usize;
    for (offset, tok) in toks[open..].iter().enumerate() {
        if tok.kind != Kind::Punct {
            continue;
        }
        if tok.text == opener {
            depth += 1;
        } else if tok.text == closer {
            depth -= 1;
            if depth == 0 {
                return Some(open + offset);
            }
        }
    }
    None
}
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 44 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/rust_scan.rs
git commit -m "feat(factory-presets): a small scanner for Rust source"
```


### Task 16: `cargo`: test ids and test regions from Rust source

**Files:**
- Create: `crates/factory-presets/src/cargo.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: `rust_scan::{tokens, matching, Kind, Tok}` (Task 15); `test_id`, `EnumerateError` (Task 11).
- Produces (both `pub(crate)`):
  - `fn enumerate_ids(path: &str, source: &str) -> Result<Vec<String>, EnumerateError>`
  - `fn test_regions(path: &str, source: &str) -> Vec<std::ops::Range<usize>>`

Assumptions A1 to A4 live in this file, in `locate` (path to class name and module path) and nowhere else in it.

- [ ] **Step 1: Write the failing tests for enumerating Rust test ids**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod cargo;
mod junit;
mod marker;
mod protected;
mod rust_scan;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/cargo.rs` holding only this test module:

<!-- plan-op: create crates/factory-presets/src/cargo.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const DISCOUNT: &str = r##"//! Percent discounts.

pub fn apply(total: u64, percent: u64) -> u64 {
    total - total * percent / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac1_ten_percent_off() {
        assert_eq!(apply(10_000, 10), 9_000);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn ac2_rejects_more_than_all() {
        apply(100, 200);
    }

    mod rounding {
        use super::*;

        #[tokio::test]
        async fn ac3_rounds_down() {
            assert_eq!(apply(999, 10), 900);
        }
    }

    fn helper() -> &'static str {
        "#[test] fn not_a_test() {}"
    }
}
"##;

    #[test]
    fn enumerates_in_file_tests_with_their_module_path() {
        assert_eq!(
            enumerate_ids("crates/cart/src/discount.rs", DISCOUNT).unwrap(),
            vec![
                "cart > discount::tests::ac1_ten_percent_off",
                "cart > discount::tests::ac2_rejects_more_than_all",
                "cart > discount::tests::rounding::ac3_rounds_down",
            ]
        );
    }

    #[test]
    fn the_class_and_module_path_follow_the_files_place_in_the_package() {
        let one = "#[test]\nfn t() {}\n";
        let id = |path: &str| enumerate_ids(path, one).unwrap().remove(0);
        assert_eq!(id("crates/cart/src/lib.rs"), "cart > t");
        assert_eq!(id("crates/cart/src/discount.rs"), "cart > discount::t");
        assert_eq!(id("crates/cart/src/pricing/mod.rs"), "cart > pricing::t");
        assert_eq!(
            id("crates/cart/src/pricing/tax.rs"),
            "cart > pricing::tax::t"
        );
        assert_eq!(
            id("crates\\cart\\src\\pricing\\tax.rs"),
            "cart > pricing::tax::t"
        );
        assert_eq!(id("crates/cart/tests/checkout.rs"), "cart::checkout > t");
        assert_eq!(
            id("crates/cart/tests/checkout/main.rs"),
            "cart::checkout > t"
        );
        assert_eq!(
            id("crates/cart/tests/checkout/steps.rs"),
            "cart::checkout > steps::t"
        );
        assert_eq!(id("crates/cart/src/main.rs"), "cart::bin/cart > t");
        assert_eq!(id("crates/cart/src/bin/report.rs"), "cart::bin/report > t");
        assert_eq!(
            id("crates/cart/src/bin/report/main.rs"),
            "cart::bin/report > t"
        );
        assert_eq!(
            id("crates/cart/src/bin/report/args.rs"),
            "cart::bin/report > args::t"
        );
    }

    #[test]
    fn a_file_with_no_package_directory_is_refused_only_if_it_declares_tests() {
        let one = "#[test]\nfn t() {}\n";
        for path in [
            "src/lib.rs",
            "tests/checkout.rs",
            "lib.rs",
            "benches/speed.rs",
        ] {
            assert_eq!(
                enumerate_ids(path, one),
                Err(EnumerateError::UnknownPackage {
                    path: path.to_string()
                })
            );
            assert_eq!(enumerate_ids(path, "pub fn f() {}\n"), Ok(vec![]));
        }
    }

    #[test]
    fn files_that_are_not_rust_and_files_without_tests_have_no_ids() {
        assert_eq!(
            enumerate_ids("crates/cart/Cargo.toml", "[package]\n"),
            Ok(vec![])
        );
        assert_eq!(
            enumerate_ids("crates/cart/tests/data/input.json", "{}"),
            Ok(vec![])
        );
        assert_eq!(enumerate_ids("crates/cart/src/lib.rs", ""), Ok(vec![]));
        assert_eq!(
            enumerate_ids(
                "crates/cart/src/lib.rs",
                "// #[test]\n/* #[test] fn x() {} */\n"
            ),
            Ok(vec![])
        );
    }

    #[test]
    fn qualified_test_functions_are_found() {
        let source = "#[test]\npub(crate) async fn a() {}\n#[test]\nunsafe extern \"C\" fn b() {}\n#[test] const fn c() {}\n";
        assert_eq!(
            enumerate_ids("crates/cart/src/lib.rs", source).unwrap(),
            vec!["cart > a", "cart > b", "cart > c"]
        );
    }

    #[test]
    fn a_test_attribute_on_something_else_declares_no_test() {
        let source = "#[test]\nstruct NotATest;\nfn plain() {}\n#[cfg(test)]\nfn helper() {}\n";
        assert_eq!(enumerate_ids("crates/cart/src/lib.rs", source), Ok(vec![]));
    }

    #[test]
    fn parametrised_tests_are_refused_with_the_attribute_that_does_it() {
        // rstest declares the tests itself; there is no `#[test]` to find.
        let rstest = "#[rstest]\n#[case(1)]\n#[case(2)]\nfn ac1_many(#[case] n: u32) {}\n";
        assert_eq!(
            enumerate_ids("crates/cart/src/lib.rs", rstest),
            Err(EnumerateError::Parametrised {
                line: 4,
                by: "rstest".to_string()
            })
        );
        let test_case = "#[test_case(1 ; \"one\")]\n#[test]\nfn ac1_many(n: u32) {}\n";
        assert_eq!(
            enumerate_ids("crates/cart/src/lib.rs", test_case)
                .unwrap_err()
                .code(),
            "parametrised"
        );
    }

    #[test]
    fn tests_whose_names_a_macro_builds_are_refused() {
        let source = r#"
macro_rules! cases {
    ($name:ident) => {
        #[test]
        fn $name() {}
    };
}
cases!(ac1_generated);
"#;
        assert_eq!(
            enumerate_ids("crates/cart/src/lib.rs", source),
            Err(EnumerateError::NotLiteral { line: 5 })
        );
        let in_module = "macro_rules! m { ($m:ident) => { mod $m { #[test] fn works() {} } }; }\n";
        assert_eq!(
            enumerate_ids("crates/cart/src/lib.rs", in_module),
            Err(EnumerateError::NotLiteral { line: 1 })
        );
    }

    #[test]
    fn the_same_test_name_twice_in_one_file_is_refused() {
        let source = "#[cfg(unix)]\n#[test]\nfn t() {}\n#[cfg(windows)]\n#[test]\nfn t() {}\n";
        assert_eq!(
            enumerate_ids("crates/cart/src/lib.rs", source),
            Err(EnumerateError::DuplicateId {
                id: "cart > t".to_string()
            })
        );
    }

    #[test]
    fn unbalanced_braces_are_refused() {
        assert_eq!(
            enumerate_ids("crates/cart/src/lib.rs", "mod a {\n#[test]\nfn t() {}\n")
                .unwrap_err()
                .code(),
            "unparseable"
        );
        assert_eq!(
            enumerate_ids("crates/cart/src/lib.rs", "}\n#[test]\nfn t() {}\n")
                .unwrap_err()
                .code(),
            "unparseable"
        );
    }

    #[test]
    fn crlf_source_enumerates_the_same_ids() {
        let crlf = DISCOUNT.replace('\n', "\r\n");
        assert_eq!(
            enumerate_ids("crates/cart/src/discount.rs", &crlf),
            enumerate_ids("crates/cart/src/discount.rs", DISCOUNT)
        );
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0433]: failed to resolve: use of undeclared type `EnumerateError`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of enumerating Rust test ids**

Add this to `crates/factory-presets/src/cargo.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/cargo.rs -->
```rust
//! The `cargo` preset's reading of Rust source: which tests a file declares, and which byte
//! ranges of it are test code.
//!
//! ASSUMPTION (spike S6 has not confirmed it): the test runner is cargo-nextest, and its
//! JUnit report gives each test case a `classname` that is the test binary's id and a `name`
//! that is the test's path inside that binary. The binary id is assumed to be the package
//! name for unit tests, `<package>::<file>` for an integration test file, and
//! `<package>::bin/<name>` for a binary target. A source file does not say what its package
//! is called, so the package name is taken from the directory that holds `src/` or `tests/`.

use crate::rust_scan::{matching, tokens, Kind, Tok};
use crate::types::{test_id, EnumerateError};
use std::ops::Range;

/// Attributes that declare tests themselves and turn one function into many of them.
const PARAMETRISING: [&str; 4] = ["rstest", "test_case", "test_matrix", "parameterized"];

/// A module whose name is not a plain identifier (`mod $name`, inside a macro definition).
const OPAQUE_MODULE: &str = "\0";

#[derive(Debug, Default)]
struct Analysis {
    /// Each test: its path inside the file (`tests::ac1_works`) and its line.
    tests: Vec<(String, usize)>,
    regions: Vec<Range<usize>>,
    /// The first reason the tests cannot be enumerated.
    fault: Option<EnumerateError>,
}

#[derive(Debug, Default)]
struct Pending {
    /// Byte offset of the first attribute before the item being read.
    start: Option<usize>,
    test: bool,
    cfg_test: bool,
    parametrised_by: Option<String>,
}

/// Read what one attribute's tokens (between `[` and `]`) say.
fn classify(attribute: &[Tok<'_>], pending: &mut Pending) {
    let path: Vec<&str> = attribute
        .iter()
        .take_while(|t| t.kind == Kind::Ident || t.text == ":")
        .filter(|t| t.kind == Kind::Ident)
        .map(|t| t.text)
        .collect();
    let (Some(first), Some(last)) = (path.first(), path.last()) else {
        return;
    };
    if *last == "test" {
        pending.test = true;
    }
    if PARAMETRISING.contains(last) {
        pending.parametrised_by.get_or_insert(last.to_string());
    }
    if *first == "cfg" && path.len() == 1 {
        let arguments = &attribute[1..];
        let names_test = arguments.iter().any(|t| t.text == "test");
        let negated = arguments.iter().any(|t| t.text == "not");
        if names_test && !negated {
            pending.cfg_test = true;
        }
    }
}

/// Byte offset one past the end of the item starting at `toks[from]`: its closing `}`, or its
/// `;` if that comes first.
fn item_end(toks: &[Tok<'_>], from: usize, source_len: usize) -> usize {
    let mut nest = 0usize;
    let mut i = from;
    while i < toks.len() {
        let tok = toks[i];
        if tok.kind == Kind::Punct {
            match tok.text {
                "(" | "[" => nest += 1,
                ")" | "]" => nest = nest.saturating_sub(1),
                ";" if nest == 0 => return tok.end,
                "{" => match matching(toks, i) {
                    Some(close) if nest == 0 => return toks[close].end,
                    Some(close) => i = close,
                    None => return source_len,
                },
                _ => {}
            }
        }
        i += 1;
    }
    source_len
}

/// Tokens that may sit between an item's attributes and its `fn` or `mod` keyword.
fn is_qualifier(tok: Tok<'_>) -> bool {
    match tok.kind {
        Kind::Ident => matches!(
            tok.text,
            "pub" | "crate" | "super" | "self" | "in" | "async" | "unsafe" | "const" | "extern"
        ),
        Kind::Punct => matches!(tok.text, "(" | ")" | ":"),
        Kind::Literal => true,
    }
}

fn analyze(source: &str) -> Analysis {
    let toks = tokens(source);
    let mut out = Analysis::default();
    let mut modules: Vec<(&str, usize)> = Vec::new();
    let mut depth = 0usize;
    let mut pending = Pending::default();
    let mut i = 0usize;

    let fault = |out: &mut Analysis, error: EnumerateError| {
        if out.fault.is_none() {
            out.fault = Some(error);
        }
    };

    while i < toks.len() {
        let tok = toks[i];

        // An attribute: `#[...]`, or the inner form `#![...]`, which belongs to no item here.
        if tok.text == "#" && tok.kind == Kind::Punct {
            let inner = toks.get(i + 1).is_some_and(|t| t.text == "!");
            let open = i + 1 + usize::from(inner);
            if toks.get(open).is_some_and(|t| t.text == "[") {
                let Some(close) = matching(&toks, open) else {
                    fault(&mut out, EnumerateError::Unparseable { line: tok.line });
                    break;
                };
                if !inner {
                    pending.start.get_or_insert(tok.start);
                    classify(&toks[open + 1..close], &mut pending);
                } else if depth == 0 {
                    // `#![cfg(test)]` at the top of a file: the whole file is test code.
                    let mut file = Pending::default();
                    classify(&toks[open + 1..close], &mut file);
                    if file.cfg_test {
                        out.regions.push(0..source.len());
                    }
                }
                i = close + 1;
                continue;
            }
        }

        if pending.start.is_some() && is_qualifier(tok) {
            i += 1;
            continue;
        }

        let attributed_from = pending.start.take();
        let attributes = std::mem::take(&mut pending);
        if let Some(start) = attributed_from {
            if attributes.cfg_test || attributes.test || attributes.parametrised_by.is_some() {
                out.regions.push(start..item_end(&toks, i, source.len()));
            }
        }

        match (tok.kind, tok.text) {
            (Kind::Ident, "fn") if attributes.test || attributes.parametrised_by.is_some() => {
                let name = toks.get(i + 1).filter(|t| t.kind == Kind::Ident);
                let opaque = modules.iter().any(|(m, _)| *m == OPAQUE_MODULE);
                match (name, attributes.parametrised_by) {
                    (_, Some(by)) => fault(
                        &mut out,
                        EnumerateError::Parametrised { line: tok.line, by },
                    ),
                    (Some(name), None) if !opaque => {
                        let mut path: Vec<&str> = modules.iter().map(|(m, _)| *m).collect();
                        path.push(name.text);
                        out.tests.push((path.join("::"), tok.line));
                    }
                    _ => fault(&mut out, EnumerateError::NotLiteral { line: tok.line }),
                }
            }
            (Kind::Ident, "mod") => {
                let name = toks.get(i + 1);
                let opens = |at: usize| toks.get(at).is_some_and(|t| t.text == "{");
                match name {
                    Some(name) if name.kind == Kind::Ident && opens(i + 2) => {
                        modules.push((name.text, depth + 1));
                    }
                    Some(name) if name.text == "$" => modules.push((OPAQUE_MODULE, depth + 1)),
                    _ => {}
                }
            }
            (Kind::Punct, "{") => depth += 1,
            (Kind::Punct, "}") => {
                if depth == 0 {
                    fault(&mut out, EnumerateError::Unparseable { line: tok.line });
                    break;
                }
                depth -= 1;
                while modules.last().is_some_and(|(_, inside)| *inside > depth) {
                    modules.pop();
                }
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 {
        let line = toks.last().map_or(1, |t| t.line);
        fault(&mut out, EnumerateError::Unparseable { line });
    }

    out.regions.sort_by_key(|r| (r.start, r.end));
    let mut merged: Vec<Range<usize>> = Vec::new();
    for region in out.regions.drain(..) {
        match merged.last_mut() {
            Some(last) if region.start <= last.end => last.end = last.end.max(region.end),
            _ => merged.push(region),
        }
    }
    out.regions = merged;
    out
}

/// Module path a file contributes: its directories and its own name, where `mod.rs`,
/// `main.rs` and `lib.rs` contribute only their directories.
fn module_path<'a>(parts: &[&'a str]) -> Vec<&'a str> {
    let Some((file, dirs)) = parts.split_last() else {
        return Vec::new();
    };
    let mut path = dirs.to_vec();
    let stem = file.strip_suffix(".rs").unwrap_or(file);
    if !matches!(stem, "mod" | "main" | "lib") {
        path.push(stem);
    }
    path
}

/// The assumed report class name for the tests in the file at `path`, and the module path
/// the file itself contributes to their names.
fn locate(path: &str) -> Result<(String, Vec<String>), EnumerateError> {
    let unknown = || EnumerateError::UnknownPackage {
        path: path.to_string(),
    };
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    let anchor = parts
        .iter()
        .position(|p| *p == "src" || *p == "tests")
        .filter(|at| *at > 0)
        .ok_or_else(unknown)?;
    let package = parts[anchor - 1];
    let rest = &parts[anchor + 1..];
    let stem = |file: &str| file.strip_suffix(".rs").unwrap_or(file).to_string();
    let (class, modules) = match (parts[anchor], rest) {
        (_, []) => return Err(unknown()),
        ("tests", [file]) => (format!("{package}::{}", stem(file)), Vec::new()),
        ("tests", [dir, inner @ ..]) => (format!("{package}::{dir}"), module_path(inner)),
        ("src", ["bin", file]) => (format!("{package}::bin/{}", stem(file)), Vec::new()),
        ("src", ["bin", dir, inner @ ..]) => (format!("{package}::bin/{dir}"), module_path(inner)),
        ("src", ["main.rs"]) => (format!("{package}::bin/{package}"), Vec::new()),
        (_, inner) => (package.to_string(), module_path(inner)),
    };
    Ok((class, modules.iter().map(|m| m.to_string()).collect()))
}

/// Test ids the Rust file at `path` declares, in source order.
pub(crate) fn enumerate_ids(path: &str, source: &str) -> Result<Vec<String>, EnumerateError> {
    if !path.to_lowercase().ends_with(".rs") {
        return Ok(Vec::new());
    }
    let analysis = analyze(source);
    if let Some(fault) = analysis.fault {
        return Err(fault);
    }
    if analysis.tests.is_empty() {
        return Ok(Vec::new());
    }
    let (class, file_modules) = locate(path)?;
    let mut ids: Vec<String> = Vec::new();
    for (name, _line) in &analysis.tests {
        let mut full = file_modules.clone();
        full.push(name.clone());
        let id = test_id(&class, &full.join("::"));
        if ids.contains(&id) {
            return Err(EnumerateError::DuplicateId { id });
        }
        ids.push(id);
    }
    Ok(ids)
}
```

`analyze` already records test regions while it walks the tokens; the next cycle exposes them. Enumeration checks for a scan fault before it looks at the path, so a file with a generated test name is refused whatever its place in the tree.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 55 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/cargo.rs
git commit -m "feat(factory-presets): enumerate cargo test ids from Rust source"
```

- [ ] **Step 6: Write the failing tests for test regions in Rust source**

Add these tests to the test module of `crates/factory-presets/src/cargo.rs`, directly above the module's closing `}` (the last line of the file):

<!-- plan-op: into-tests crates/factory-presets/src/cargo.rs -->
```rust
    #[test]
    fn the_cfg_test_module_is_one_region_from_its_attribute_to_its_brace() {
        let regions = test_regions("crates/cart/src/discount.rs", DISCOUNT);
        assert_eq!(regions.len(), 1);
        let text = &DISCOUNT[regions[0].clone()];
        assert!(text.starts_with("#[cfg(test)]\nmod tests {"));
        assert!(text.ends_with("\"#[test] fn not_a_test() {}\"\n    }\n}"));
        assert!(!text.contains("pub fn apply"));
        assert_eq!(regions[0].end, DISCOUNT.len() - 1);
    }

    #[test]
    fn a_bare_test_function_and_other_cfg_test_items_are_regions_too() {
        let source = "fn code() {}\n\n#[test]\nfn bare() {\n    code();\n}\n\n#[cfg(test)]\nuse std::fmt;\n\n#[cfg(all(test, unix))]\nfn helper() -> u8 { 1 }\n\nfn more() {}\n";
        let texts: Vec<&str> = test_regions("crates/cart/src/lib.rs", source)
            .into_iter()
            .map(|r| &source[r])
            .collect();
        assert_eq!(
            texts,
            vec![
                "#[test]\nfn bare() {\n    code();\n}",
                "#[cfg(test)]\nuse std::fmt;",
                "#[cfg(all(test, unix))]\nfn helper() -> u8 { 1 }",
            ]
        );
    }

    #[test]
    fn code_compiled_only_outside_tests_is_not_a_region() {
        let source = "#![allow(dead_code)]\n#[cfg(not(test))]\nfn real() {}\n#[cfg(feature = \"x\")]\nfn gated() {}\n";
        assert!(test_regions("crates/cart/src/lib.rs", source).is_empty());
    }

    #[test]
    fn a_file_level_cfg_test_makes_the_whole_file_one_region() {
        let source = "#![cfg(test)]\nuse super::*;\n\n#[test]\nfn t() {}\n";
        assert_eq!(
            test_regions("crates/cart/src/discount/tests.rs", source),
            vec![0..source.len()]
        );
    }

    #[test]
    fn a_file_with_no_test_code_or_that_is_not_rust_has_no_regions() {
        assert!(test_regions("crates/cart/src/lib.rs", "pub fn f() {}\n").is_empty());
        assert!(test_regions("crates/cart/src/lib.rs", "").is_empty());
        assert!(test_regions("README.md", "#[cfg(test)]\nmod tests {}\n").is_empty());
    }

    #[test]
    fn regions_are_sorted_and_do_not_overlap() {
        let source =
            "#[cfg(test)]\nmod a {\n    #[test]\n    fn inner() {}\n}\n#[cfg(test)]\nmod b {}\n";
        let regions = test_regions("crates/cart/src/lib.rs", source);
        assert_eq!(regions.len(), 2);
        assert!(regions[0].end <= regions[1].start);
        assert_eq!(&source[regions[1].clone()], "#[cfg(test)]\nmod b {}");
    }
```

- [ ] **Step 7: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find function `test_regions` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 8: Write the implementation of test regions in Rust source**

Add this to `crates/factory-presets/src/cargo.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/cargo.rs -->
```rust
/// Byte ranges of test code in a Rust file: every item under `#[cfg(test)]`, and every
/// `#[test]` function outside one, each from its first attribute to its closing brace.
pub(crate) fn test_regions(path: &str, source: &str) -> Vec<Range<usize>> {
    if !path.to_lowercase().ends_with(".rs") {
        return Vec::new();
    }
    analyze(source).regions
}
```

The scan did the work in the previous cycle; this adds the function that hands the ranges out.

- [ ] **Step 9: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 61 passed; 0 failed`.

- [ ] **Step 10: Commit**

```bash
git add crates/factory-presets/src/cargo.rs
git commit -m "feat(factory-presets): byte ranges of in-file Rust test code"
```


### Task 17: A scanner for JavaScript and TypeScript source

**Files:**
- Create: `crates/factory-presets/src/js_scan.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces (all `pub(crate)`):
  - `enum Kind { Word, Str { literal: bool }, Regex, Punct }` — `literal` is true for a quoted string and for a template literal with no `${...}`
  - `struct Tok<'a> { kind: Kind, text: &'a str, start: usize, end: usize, line: usize }`
  - `fn tokens(src: &str) -> Result<Vec<Tok<'_>>, usize>` — `Err` is the line of a string, template, comment or regular expression that never ends
  - `fn unquote(tok: &Tok<'_>) -> Option<String>` — the value of a literal string; `None` for an escape this crate does not resolve
  - `fn matching(toks: &[Tok<'_>], open: usize) -> Option<usize>`

- [ ] **Step 1: Write the failing tests for the JavaScript scanner**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod cargo;
mod js_scan;
mod junit;
mod marker;
mod protected;
mod rust_scan;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/js_scan.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/js_scan.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<&str> {
        tokens(src).unwrap().iter().map(|t| t.text).collect()
    }

    #[test]
    fn splits_words_strings_and_punctuation() {
        assert_eq!(
            texts("describe('cart', () => { it(\"ac1 adds\", fn); });"),
            vec![
                "describe",
                "(",
                "'cart'",
                ",",
                "(",
                ")",
                "=",
                ">",
                "{",
                "it",
                "(",
                "\"ac1 adds\"",
                ",",
                "fn",
                ")",
                ";",
                "}",
                ")",
                ";"
            ]
        );
    }

    #[test]
    fn comments_are_dropped_and_lines_are_counted() {
        let toks = tokens("// it('no')\n/* it('no')\n */ it('yes')").unwrap();
        assert_eq!(toks.len(), 4);
        assert_eq!(toks[0].text, "it");
        assert_eq!(toks[0].line, 3);
    }

    #[test]
    fn a_string_holding_brackets_and_quotes_is_one_token() {
        let toks = tokens(r#"x("a ) ' b", 'c \' ( d')"#).unwrap();
        let strings: Vec<&str> = toks
            .iter()
            .filter(|t| matches!(t.kind, Kind::Str { .. }))
            .map(|t| t.text)
            .collect();
        assert_eq!(strings, vec![r#""a ) ' b""#, r"'c \' ( d'"]);
    }

    #[test]
    fn a_template_is_literal_only_without_interpolation() {
        let toks = tokens("a(`plain ) text`); b(`n=${f(`inner ${1}`, '}')} done`);").unwrap();
        let kinds: Vec<Kind> = toks
            .iter()
            .filter(|t| matches!(t.kind, Kind::Str { .. }))
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![Kind::Str { literal: true }, Kind::Str { literal: false }]
        );
        assert_eq!(toks.iter().filter(|t| t.text == "(").count(), 2);
    }

    #[test]
    fn a_regular_expression_is_one_token_and_division_is_not_one() {
        let toks = tokens(r"const re = /it\('x'\)[/)]/gi; const half = total / 2 / n;").unwrap();
        let regexes: Vec<&str> = toks
            .iter()
            .filter(|t| t.kind == Kind::Regex)
            .map(|t| t.text)
            .collect();
        assert_eq!(regexes, vec![r"/it\('x'\)[/)]/gi"]);
        assert_eq!(toks.iter().filter(|t| t.text == "/").count(), 2);
        let after_paren = tokens("f(a) / 2; x = (b) / c / d").unwrap();
        assert!(!after_paren.iter().any(|t| t.kind == Kind::Regex));
        let after_return = tokens("return /ab+c/.test(s)").unwrap();
        assert_eq!(after_return[1].kind, Kind::Regex);
    }

    #[test]
    fn text_that_never_closes_is_an_error_with_its_line() {
        assert_eq!(tokens("a\nb('never closed\n)"), Err(2));
        assert_eq!(tokens("a(`never closed"), Err(1));
        assert_eq!(tokens("\n\n/* never closed"), Err(3));
        assert_eq!(tokens("x = /never closed\n"), Err(1));
        assert_eq!(tokens("a(`${ never closed`"), Err(1));
        assert_eq!(tokens(""), Ok(vec![]));
    }

    #[test]
    fn unquote_reads_fixed_text_and_refuses_what_it_cannot_resolve() {
        let value = |src: &str| unquote(&tokens(src).unwrap()[0]);
        assert_eq!(value("'ac1 adds'"), Some("ac1 adds".to_string()));
        assert_eq!(value("\"it's\""), Some("it's".to_string()));
        assert_eq!(value(r"'it\'s \\ \` ok'"), Some(r"it's \ ` ok".to_string()));
        assert_eq!(value("`plain`"), Some("plain".to_string()));
        assert_eq!(
            value("'caf\u{e9} \u{2713}'"),
            Some("caf\u{e9} \u{2713}".to_string())
        );
        assert_eq!(value("''"), Some(String::new()));
        assert_eq!(value(r"'tab\there'"), None);
        assert_eq!(value(r"'\x41'"), None);
        assert_eq!(value("`n=${n}`"), None);
        assert_eq!(value("word"), None);
    }

    #[test]
    fn matching_finds_the_closing_bracket_of_the_same_kind() {
        let toks = tokens("if (import.meta.vitest) { a({ b: [1] }) } c").unwrap();
        let open = toks.iter().position(|t| t.text == "{").unwrap();
        let close = matching(&toks, open).unwrap();
        assert_eq!(toks[close + 1].text, "c");
        assert_eq!(matching(&toks, 0), None);
        assert_eq!(matching(&tokens("( (").unwrap(), 0), None);
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find function `tokens` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the JavaScript scanner**

Add this to `crates/factory-presets/src/js_scan.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/js_scan.rs -->
```rust
//! A small scanner for JavaScript and TypeScript source: enough to find calls such as
//! `describe('name', ...)` and matching brackets, with comments, strings, template literals
//! and regular expressions out of the way.
//!
//! It is not a JavaScript parser. JSX is not supported; the node preset refuses `.jsx` and
//! `.tsx` test files before it gets here.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// An identifier, keyword or number.
    Word,
    /// A string. `literal` is true when its value is fixed text: a quoted string, or a
    /// template literal with no `${...}`.
    Str { literal: bool },
    /// A regular expression literal.
    Regex,
    /// One punctuation character.
    Punct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tok<'a> {
    pub kind: Kind,
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
    /// 1-based line of the token's first byte.
    pub line: usize,
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
}

/// The index one past a quoted string opened at `open`, or `None` if the line ends first.
fn end_of_quoted(bytes: &[u8], open: usize) -> Option<usize> {
    let quote = bytes[open];
    let mut i = open + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'\n' => return None,
            b if b == quote => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// The index one past a template literal opened at `open`, and whether it has a `${...}`.
fn end_of_template(bytes: &[u8], open: usize) -> Option<(usize, bool)> {
    let mut i = open + 1;
    let mut interpolated = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'`' => return Some((i + 1, interpolated)),
            b'$' if bytes.get(i + 1) == Some(&b'{') => {
                interpolated = true;
                i = end_of_interpolation(bytes, i + 2)?;
            }
            _ => i += 1,
        }
    }
    None
}

/// The index one past the `}` that closes a `${` whose body starts at `from`.
fn end_of_interpolation(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            b'\'' | b'"' => {
                i = end_of_quoted(bytes, i)?;
                continue;
            }
            b'`' => {
                i = end_of_template(bytes, i)?.0;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The index one past a regular expression literal opened at `open`.
fn end_of_regex(bytes: &[u8], open: usize) -> Option<usize> {
    let mut i = open + 1;
    let mut in_class = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'\n' => return None,
            b'[' => in_class = true,
            b']' => in_class = false,
            b'/' if !in_class => {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                return Some(i);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Whether a `/` after `previous` starts a regular expression rather than a division.
fn regex_may_follow(previous: Option<&Tok<'_>>) -> bool {
    match previous {
        None => true,
        Some(tok) => match tok.kind {
            Kind::Punct => !matches!(tok.text, ")" | "]" | "}"),
            Kind::Word => matches!(
                tok.text,
                "return"
                    | "typeof"
                    | "case"
                    | "do"
                    | "else"
                    | "in"
                    | "of"
                    | "void"
                    | "delete"
                    | "throw"
                    | "new"
                    | "yield"
                    | "await"
            ),
            Kind::Str { .. } | Kind::Regex => false,
        },
    }
}

/// The tokens of `src`, in order, without comments or whitespace. `Err` carries the line of
/// a string, template, comment or regular expression that never ends.
pub(crate) fn tokens(src: &str) -> Result<Vec<Tok<'_>>, usize> {
    let bytes = src.as_bytes();
    let mut out: Vec<Tok<'_>> = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if byte.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            let Some(length) = src[i + 2..].find("*/") else {
                return Err(line);
            };
            let end = i + 2 + length + 2;
            line += bytes[i..end].iter().filter(|b| **b == b'\n').count();
            i = end;
            continue;
        }

        let start = i;
        let (end, kind) = match byte {
            b'\'' | b'"' => (
                end_of_quoted(bytes, i).ok_or(line)?,
                Kind::Str { literal: true },
            ),
            b'`' => {
                let (end, interpolated) = end_of_template(bytes, i).ok_or(line)?;
                (
                    end,
                    Kind::Str {
                        literal: !interpolated,
                    },
                )
            }
            b'/' if regex_may_follow(out.last()) => {
                (end_of_regex(bytes, i).ok_or(line)?, Kind::Regex)
            }
            b if is_word(b) => {
                let mut end = i;
                while end < bytes.len() && is_word(bytes[end]) {
                    end += 1;
                }
                (end, Kind::Word)
            }
            _ => (i + 1, Kind::Punct),
        };
        let end = end.min(bytes.len());
        out.push(Tok {
            kind,
            text: &src[start..end],
            start,
            end,
            line,
        });
        line += bytes[start..end].iter().filter(|b| **b == b'\n').count();
        i = end;
    }
    Ok(out)
}

/// The value of a literal string token: its text without the quotes, with the escapes
/// `\\`, `\'`, `\"` and `` \` `` resolved. `None` for any other escape: this crate does not
/// guess what a runner would print for it.
pub(crate) fn unquote(tok: &Tok<'_>) -> Option<String> {
    if tok.kind != (Kind::Str { literal: true }) || tok.text.len() < 2 {
        return None;
    }
    let inner = &tok.text[1..tok.text.len() - 1];
    let mut value = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            value.push(ch);
            continue;
        }
        match chars.next() {
            Some(escaped @ ('\\' | '\'' | '"' | '`')) => value.push(escaped),
            _ => return None,
        }
    }
    Some(value)
}

/// The index of the token that closes the bracket opened at `open`, counting only that kind
/// of bracket.
pub(crate) fn matching(toks: &[Tok<'_>], open: usize) -> Option<usize> {
    let opener = toks.get(open)?.text;
    let closer = match opener {
        "(" => ")",
        "[" => "]",
        "{" => "}",
        _ => return None,
    };
    let mut depth = 0usize;
    for (offset, tok) in toks[open..].iter().enumerate() {
        if tok.kind != Kind::Punct {
            continue;
        }
        if tok.text == opener {
            depth += 1;
        } else if tok.text == closer {
            depth -= 1;
            if depth == 0 {
                return Some(open + offset);
            }
        }
    }
    None
}
```

`unquote` resolves only the four escapes whose meaning cannot be argued about. A name written with `\n`, `\t` or a `\x..` escape is refused rather than guessed at: the id must equal what the runner prints, byte for byte.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 69 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/js_scan.rs
git commit -m "feat(factory-presets): a small scanner for JavaScript and TypeScript source"
```


### Task 18: `node`: test ids and test regions from JS/TS source

**Files:**
- Create: `crates/factory-presets/src/node.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: `js_scan::{tokens, unquote, matching, Kind, Tok}` (Task 17); `EnumerateError`, `ID_SEPARATOR` (Task 11).
- Produces (both `pub(crate)`):
  - `fn enumerate_ids(path: &str, source: &str) -> Result<Vec<String>, EnumerateError>`
  - `fn test_regions(path: &str, source: &str) -> Vec<std::ops::Range<usize>>`

One enumerator serves vitest and `node --test`: both declare tests with `describe`/`it`/`test` calls. Assumption A5/A7 here is only that the id starts with the file's repository-relative path.

- [ ] **Step 1: Write the failing tests for enumerating JavaScript test ids**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod cargo;
mod js_scan;
mod junit;
mod marker;
mod node;
mod protected;
mod rust_scan;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/node.rs` holding only this test module:

<!-- plan-op: create crates/factory-presets/src/node.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const VITEST: &str = r#"import { describe, it, expect } from 'vitest'
import { total } from './cart'

describe('cart', () => {
  it('ac1 applies a percent code', () => {
    expect(total(10000, 'SAVE10')).toBe(9000)
  })

  describe("unknown codes", function () {
    it.skip(`ac2 leaves the total unchanged`, async () => {
      expect(total(10000, 'NOPE')).toBe(10000)
    })
  })

  test('totals are never negative (ac3)', () => {})
})

test('top level', () => {})
"#;

    #[test]
    fn enumerates_tests_with_their_describe_chain() {
        assert_eq!(
            enumerate_ids("src/cart.test.ts", VITEST).unwrap(),
            vec![
                "src/cart.test.ts > cart > ac1 applies a percent code",
                "src/cart.test.ts > cart > unknown codes > ac2 leaves the total unchanged",
                "src/cart.test.ts > cart > totals are never negative (ac3)",
                "src/cart.test.ts > top level",
            ]
        );
    }

    #[test]
    fn node_test_source_reads_the_same_way() {
        let source = r#"const { describe, it } = require('node:test')
const assert = require('node:assert/strict')

describe('cart', () => {
  it('ac1 applies a percent code', () => {
    assert.equal(total(10000, 'SAVE10'), 9000)
  })
})
"#;
        assert_eq!(
            enumerate_ids("test\\cart.test.js", source).unwrap(),
            vec!["test/cart.test.js > cart > ac1 applies a percent code"]
        );
    }

    #[test]
    fn crlf_source_enumerates_the_same_ids() {
        let crlf = VITEST.replace('\n', "\r\n");
        assert_eq!(
            enumerate_ids("src/cart.test.ts", &crlf),
            enumerate_ids("src/cart.test.ts", VITEST)
        );
    }

    #[test]
    fn mentions_that_are_not_calls_declare_nothing() {
        let source = r#"import test from 'node:test'
// it('commented out', () => {})
const s = "it('in a string', () => {})"
const ok = /^it\('x'\)$/.test(s)
const o = { test: 1, it: 2 }
function it(name) { return name }
runner.test('member call', () => {})
"#;
        assert_eq!(enumerate_ids("test/a.test.js", source), Ok(vec![]));
    }

    #[test]
    fn a_name_that_is_not_a_plain_literal_is_refused() {
        let refused = [
            "it(name, () => {})",
            "it(`ac1 case ${n}`, () => {})",
            "it('ac1 ' + suffix, () => {})",
            "it('tab\\there', () => {})",
            "describe(makeName(), () => { it('x', () => {}) })",
            "it()",
        ];
        for source in refused {
            assert_eq!(
                enumerate_ids("test/a.test.js", source),
                Err(EnumerateError::NotLiteral { line: 1 }),
                "{source}"
            );
        }
    }

    #[test]
    fn a_dynamic_name_anywhere_fails_the_whole_file_not_just_that_test() {
        let source = "it('ac1 fine', () => {})\nfor (const n of [1, 2]) {\n  it(`ac2 case ${n}`, () => {})\n}\n";
        assert_eq!(
            enumerate_ids("test/a.test.js", source),
            Err(EnumerateError::NotLiteral { line: 3 })
        );
    }

    #[test]
    fn parametrised_declarations_are_refused_with_the_call_that_does_it() {
        let cases = [
            ("it.each([1, 2])('ac1 case %i', (n) => {})", "it.each"),
            (
                "test.each`\n a | b\n ${1} | ${2}\n`('ac1 $a', () => {})",
                "test.each",
            ),
            ("describe.each([1])('group %i', () => {})", "describe.each"),
            ("it.skipIf(process.env.CI)('ac1 x', () => {})", "it.skipIf"),
            ("test.concurrent.for([1])('ac1 %i', () => {})", "test.for"),
        ];
        for (source, by) in cases {
            assert_eq!(
                enumerate_ids("test/a.test.js", source),
                Err(EnumerateError::Parametrised {
                    line: 1,
                    by: by.to_string()
                }),
                "{source}"
            );
        }
    }

    #[test]
    fn plain_modifiers_still_declare_one_test() {
        let source = "describe.only('g', () => {\n  it.todo('ac1 later')\n  test.concurrent('ac2 now', async () => {})\n  it.fails('ac3 known', () => {})\n})\n";
        assert_eq!(
            enumerate_ids("test/a.test.js", source).unwrap(),
            vec![
                "test/a.test.js > g > ac1 later",
                "test/a.test.js > g > ac2 now",
                "test/a.test.js > g > ac3 known",
            ]
        );
    }

    #[test]
    fn a_property_read_on_a_variable_named_test_is_not_a_call() {
        let source = "for (const test of cases) { run(test.input, test.expected) }\n";
        assert_eq!(enumerate_ids("test/a.test.js", source), Ok(vec![]));
    }

    #[test]
    fn the_same_test_declared_twice_is_refused() {
        let source = "it('ac1 same', () => {})\nit('ac1 same', () => {})\n";
        assert_eq!(
            enumerate_ids("test/a.test.js", source),
            Err(EnumerateError::DuplicateId {
                id: "test/a.test.js > ac1 same".to_string()
            })
        );
        let apart = "describe('a', () => { it('same', () => {}) })\ndescribe('b', () => { it('same', () => {}) })\n";
        assert_eq!(enumerate_ids("test/a.test.js", apart).unwrap().len(), 2);
    }

    #[test]
    fn jsx_test_files_are_refused_and_other_files_have_no_ids() {
        for path in ["src/App.test.tsx", "src/App.test.JSX"] {
            assert_eq!(
                enumerate_ids(path, "it('x', () => {})"),
                Err(EnumerateError::UnsupportedFile {
                    path: path.to_string()
                })
            );
        }
        for path in [
            "test/fixtures/data.json",
            "test/README.md",
            "test/snap.snap",
            "Makefile",
        ] {
            assert_eq!(enumerate_ids(path, "it('x', () => {})"), Ok(vec![]));
        }
        assert_eq!(enumerate_ids("test/empty.test.mjs", ""), Ok(vec![]));
    }

    #[test]
    fn source_that_does_not_scan_is_refused() {
        assert_eq!(
            enumerate_ids("test/a.test.js", "it('never closed\n"),
            Err(EnumerateError::Unparseable { line: 1 })
        );
        assert_eq!(
            enumerate_ids(
                "test/a.test.js",
                "describe('g', () => {\n  it('x', () => {})\n"
            )
            .unwrap_err()
            .code(),
            "unparseable"
        );
        assert_eq!(
            enumerate_ids("test/a.test.js", ")\n").unwrap_err().code(),
            "unparseable"
        );
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find function `enumerate_ids` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of enumerating JavaScript test ids**

Add this to `crates/factory-presets/src/node.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/node.rs -->
```rust
//! The `node` preset's reading of JavaScript and TypeScript test source, for vitest and for
//! `node --test`: which tests a file declares, and which byte ranges are in-file test code.
//!
//! ASSUMPTION (spike S6 has not confirmed it): a test's id is its file's repository-relative
//! path, then the names of the `describe` blocks around it, then its own name, joined by
//! `" > "`. Both runners are assumed to report enough to rebuild exactly that.

use crate::js_scan::{matching, tokens, unquote, Kind, Tok};
use crate::types::{EnumerateError, ID_SEPARATOR};
use std::ops::Range;

/// Calls that open a group of tests.
const GROUPS: [&str; 2] = ["describe", "suite"];
/// Calls that declare one test.
const TESTS: [&str; 2] = ["it", "test"];
/// Modifiers that leave a call declaring exactly one statically named thing.
const PLAIN_MODIFIERS: [&str; 6] = ["skip", "only", "todo", "concurrent", "sequential", "fails"];

enum FileKind {
    Script,
    Jsx,
    Other,
}

fn file_kind(path: &str) -> FileKind {
    let lower = path.to_lowercase();
    match lower.rsplit('.').next() {
        Some("js" | "mjs" | "cjs" | "ts" | "mts" | "cts") => FileKind::Script,
        Some("jsx" | "tsx") => FileKind::Jsx,
        _ => FileKind::Other,
    }
}

fn is_punct(tok: Option<&Tok<'_>>, text: &str) -> bool {
    tok.is_some_and(|t| t.kind == Kind::Punct && t.text == text)
}

/// Test ids the file at `path` declares, in source order.
pub(crate) fn enumerate_ids(path: &str, source: &str) -> Result<Vec<String>, EnumerateError> {
    match file_kind(path) {
        FileKind::Script => {}
        FileKind::Other => return Ok(Vec::new()),
        FileKind::Jsx => {
            return Err(EnumerateError::UnsupportedFile {
                path: path.to_string(),
            })
        }
    }
    let class = path.replace('\\', "/");
    let toks = tokens(source).map_err(|line| EnumerateError::Unparseable { line })?;
    let mut ids: Vec<String> = Vec::new();
    // Open groups: each name, and the parenthesis depth its call was made at.
    let mut groups: Vec<(String, usize)> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < toks.len() {
        let tok = &toks[i];
        match tok.kind {
            Kind::Punct if tok.text == "(" => depth += 1,
            Kind::Punct if tok.text == ")" => {
                if depth == 0 {
                    return Err(EnumerateError::Unparseable { line: tok.line });
                }
                depth -= 1;
                while groups.last().is_some_and(|(_, at)| *at == depth) {
                    groups.pop();
                }
            }
            Kind::Word if GROUPS.contains(&tok.text) || TESTS.contains(&tok.text) => {
                let previous = i.checked_sub(1).map(|p| &toks[p]);
                let is_member = is_punct(previous, ".");
                let is_declaration =
                    previous.is_some_and(|p| p.kind == Kind::Word && p.text == "function");
                if !is_member && !is_declaration {
                    // Skip `.modifier` links, remembering the first one that is not plain.
                    let mut j = i + 1;
                    let mut odd: Option<&str> = None;
                    while is_punct(toks.get(j), ".")
                        && toks.get(j + 1).is_some_and(|t| t.kind == Kind::Word)
                    {
                        let modifier = toks[j + 1].text;
                        if !PLAIN_MODIFIERS.contains(&modifier) {
                            odd.get_or_insert(modifier);
                        }
                        j += 2;
                    }
                    let tagged = toks.get(j).is_some_and(|t| {
                        matches!(t.kind, Kind::Str { .. }) && t.text.starts_with('`')
                    });
                    let called = is_punct(toks.get(j), "(");
                    if let (Some(by), true) = (odd, called || tagged) {
                        return Err(EnumerateError::Parametrised {
                            line: tok.line,
                            by: format!("{}.{by}", tok.text),
                        });
                    }
                    if called {
                        let ends_argument =
                            is_punct(toks.get(j + 2), ",") || is_punct(toks.get(j + 2), ")");
                        let name = toks
                            .get(j + 1)
                            .filter(|_| ends_argument)
                            .and_then(unquote)
                            .ok_or(EnumerateError::NotLiteral { line: tok.line })?;
                        if GROUPS.contains(&tok.text) {
                            groups.push((name, depth));
                        } else {
                            let mut parts: Vec<&str> = vec![class.as_str()];
                            parts.extend(groups.iter().map(|(group, _)| group.as_str()));
                            parts.push(name.as_str());
                            let id = parts.join(ID_SEPARATOR);
                            if ids.contains(&id) {
                                return Err(EnumerateError::DuplicateId { id });
                            }
                            ids.push(id);
                        }
                        depth += 1;
                        i = j + 2;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 {
        let line = toks.last().map_or(1, |t| t.line);
        return Err(EnumerateError::Unparseable { line });
    }
    Ok(ids)
}
```

Until the next cycle, the compiler warns that `matching` and `Range` are imported and unused. That is expected. Note the order of the checks on a call: a parametrising modifier is reported as such even when the name after it is a fine literal, because one declaration is then many tests.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 81 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/node.rs
git commit -m "feat(factory-presets): enumerate node test ids from JavaScript and TypeScript source"
```

- [ ] **Step 6: Write the failing tests for vitest in-source test blocks**

Add these tests to the test module of `crates/factory-presets/src/node.rs`, directly above the module's closing `}` (the last line of the file):

<!-- plan-op: into-tests crates/factory-presets/src/node.rs -->
```rust
    #[test]
    fn a_vitest_in_source_block_is_a_region() {
        let source = "export function add(a, b) {\n  return a + b\n}\n\nif (import.meta.vitest) {\n  const { it, expect } = import.meta.vitest\n  it('ac1 adds', () => { expect(add(1, 2)).toBe(3) })\n}\n";
        let regions = test_regions("src/add.ts", source);
        assert_eq!(regions.len(), 1);
        let text = &source[regions[0].clone()];
        assert!(text.starts_with("if (import.meta.vitest) {"));
        assert!(text.ends_with("toBe(3) })\n}"));
        assert_eq!(regions[0].end, source.len() - 1);
    }

    #[test]
    fn files_without_an_in_source_block_have_no_regions() {
        assert!(test_regions("src/add.ts", "export const x = import.meta.env\n").is_empty());
        assert!(test_regions("src/add.ts", "").is_empty());
        assert!(test_regions("src/add.ts", "if (import.meta.vitest) {").is_empty());
        assert!(test_regions("src/add.ts", "const s = 'never closed\n").is_empty());
        assert!(test_regions("README.md", "if (import.meta.vitest) { }").is_empty());
    }
```

- [ ] **Step 7: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find function `test_regions` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 8: Write the implementation of vitest in-source test blocks**

Add this to `crates/factory-presets/src/node.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/node.rs -->
```rust
/// Byte ranges of in-file test code: each vitest in-source block,
/// `if (import.meta.vitest) { ... }`, from its `if` to its closing brace.
pub(crate) fn test_regions(path: &str, source: &str) -> Vec<Range<usize>> {
    if !matches!(file_kind(path), FileKind::Script) {
        return Vec::new();
    }
    let Ok(toks) = tokens(source) else {
        return Vec::new();
    };
    const GUARD: [&str; 8] = ["if", "(", "import", ".", "meta", ".", "vitest", ")"];
    let mut regions = Vec::new();
    let mut i = 0usize;
    while i + GUARD.len() < toks.len() {
        let is_guard = GUARD
            .iter()
            .enumerate()
            .all(|(offset, text)| toks[i + offset].text == *text);
        let open = i + GUARD.len();
        if is_guard && is_punct(toks.get(open), "{") {
            if let Some(close) = matching(&toks, open) {
                regions.push(toks[i].start..toks[close].end);
                i = close + 1;
                continue;
            }
        }
        i += 1;
    }
    regions
}
```

A file that does not scan has no regions here; it is `enumerate_ids` that refuses such a file.

- [ ] **Step 9: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 83 passed; 0 failed`.

- [ ] **Step 10: Commit**

```bash
git add crates/factory-presets/src/node.rs
git commit -m "feat(factory-presets): byte ranges of vitest in-source test blocks"
```


### Task 19: Reading dependency manifests

**Files:**
- Create: `crates/factory-presets/src/manifest.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: `ManifestError`, `Side` (Task 11); `toml::{Table, Value}`; `serde_json::{Map, Value}`.
- Produces (both `pub(crate)`):
  - `fn cargo_added(path: &str, before: &str, after: &str) -> Result<Vec<String>, ManifestError>`
  - `fn node_added(path: &str, before: &str, after: &str) -> Result<Vec<String>, ManifestError>`

  Each returns the added dependency names, sorted and without repeats.

- [ ] **Step 1: Write the failing tests for reading `Cargo.toml`**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod cargo;
mod js_scan;
mod junit;
mod manifest;
mod marker;
mod node;
mod protected;
mod rust_scan;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/manifest.rs` holding only this test module:

<!-- plan-op: create crates/factory-presets/src/manifest.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const CARGO: &str = r#"[package]
name = "cart"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
proptest = "1"
"#;

    fn cargo(after: &str) -> Result<Vec<String>, ManifestError> {
        cargo_added("crates/cart/Cargo.toml", CARGO, after)
    }

    fn other_change(key: &str) -> Result<Vec<String>, ManifestError> {
        Err(ManifestError::OtherChange {
            key: key.to_string(),
        })
    }

    #[test]
    fn an_unchanged_or_reformatted_cargo_manifest_adds_nothing() {
        assert_eq!(cargo(CARGO), Ok(vec![]));
        let reformatted = format!("# a comment\n{}", CARGO.replace(" = ", "="));
        assert_eq!(cargo(&reformatted), Ok(vec![]));
        assert_eq!(cargo(&CARGO.replace('\n', "\r\n")), Ok(vec![]));
        assert_eq!(cargo(&format!("\u{feff}{CARGO}")), Ok(vec![]));
    }

    #[test]
    fn added_cargo_dependencies_are_listed_sorted_from_every_table() {
        let after = CARGO
            .replace(
                "[dev-dependencies]",
                "rust_decimal = \"1\"\n\n[dev-dependencies]",
            )
            .replace(
                "proptest = \"1\"",
                "proptest = \"1\"\ninsta = { version = \"1\" }",
            )
            + "\n[build-dependencies]\ncc = \"1\"\n";
        assert_eq!(
            cargo(&after),
            Ok(vec![
                "cc".to_string(),
                "insta".to_string(),
                "rust_decimal".to_string()
            ])
        );
    }

    #[test]
    fn workspace_dependencies_are_read_in_both_forms() {
        let before =
            "[workspace]\nmembers = [\"crates/cart\"]\n\n[workspace.dependencies]\nserde = \"1\"\n";
        let after = format!("{before}rust_decimal = \"1\"\n");
        assert_eq!(
            cargo_added("Cargo.toml", before, &after),
            Ok(vec!["rust_decimal".to_string()])
        );
        let member = CARGO.replace(
            "[dev-dependencies]",
            "rust_decimal = { workspace = true }\n\n[dev-dependencies]",
        );
        assert_eq!(cargo(&member), Ok(vec!["rust_decimal".to_string()]));
    }

    #[test]
    fn a_renamed_cargo_dependency_is_reported_by_its_real_name() {
        let after = CARGO.replace(
            "[dev-dependencies]",
            "decimal = { package = \"rust_decimal\", version = \"1\" }\n\n[dev-dependencies]",
        );
        assert_eq!(cargo(&after), Ok(vec!["rust_decimal".to_string()]));
    }

    #[test]
    fn an_added_cargo_dependency_from_a_path_git_or_other_registry_is_refused() {
        for entry in [
            "evil = { path = \"../evil\" }",
            "evil = { git = \"https://example.com/evil.git\" }",
            "evil = { version = \"1\", registry = \"mirror\" }",
        ] {
            let after = CARGO.replace(
                "[dev-dependencies]",
                &format!("{entry}\n\n[dev-dependencies]"),
            );
            assert_eq!(
                cargo(&after),
                Err(ManifestError::UnsupportedSource {
                    name: "evil".to_string()
                })
            );
        }
    }

    #[test]
    fn removing_or_changing_a_cargo_dependency_is_refused() {
        assert_eq!(
            cargo(&CARGO.replace("proptest = \"1\"\n", "")),
            Err(ManifestError::DependencyRemoved {
                name: "proptest".to_string()
            })
        );
        assert_eq!(
            cargo(&CARGO.replace("features = [\"derive\"]", "features = [\"derive\", \"rc\"]")),
            Err(ManifestError::DependencyChanged {
                name: "serde".to_string()
            })
        );
        assert_eq!(
            cargo(&CARGO.replace("proptest = \"1\"", "proptest = \"2\"")),
            Err(ManifestError::DependencyChanged {
                name: "proptest".to_string()
            })
        );
    }

    #[test]
    fn any_other_cargo_change_is_refused_and_named() {
        assert_eq!(
            cargo(&CARGO.replace(
                "edition = \"2021\"",
                "edition = \"2021\"\nbuild = \"build.rs\""
            )),
            other_change("package.build")
        );
        assert_eq!(
            cargo(&format!("{CARGO}\n[features]\nfast = []\n")),
            other_change("features")
        );
        assert_eq!(
            cargo(&format!(
                "{CARGO}\n[[test]]\nname = \"only\"\npath = \"tests/only.rs\"\n"
            )),
            other_change("test")
        );
        assert_eq!(
            cargo(&format!("{CARGO}\n[profile.test]\nopt-level = 3\n")),
            other_change("profile")
        );
        assert_eq!(
            cargo(&format!(
                "{CARGO}\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n"
            )),
            other_change("target")
        );
        assert_eq!(
            cargo(&CARGO.replace("name = \"cart\"", "name = \"kart\"")),
            other_change("package.name")
        );
    }

    #[test]
    fn a_cargo_manifest_that_does_not_parse_is_refused_with_its_side() {
        assert!(matches!(
            cargo("[package\n"),
            Err(ManifestError::Malformed {
                side: Side::After,
                ..
            })
        ));
        assert!(matches!(
            cargo_added("Cargo.toml", "[package\n", CARGO),
            Err(ManifestError::Malformed {
                side: Side::Before,
                ..
            })
        ));
        assert!(matches!(
            cargo_added(
                "Cargo.toml",
                "[package]\nname = \"x\"\n",
                "dependencies = 3\n\n[package]\nname = \"x\"\n"
            ),
            Err(ManifestError::Malformed {
                side: Side::After,
                ..
            })
        ));
        // A key written twice is a parse error, not a choice between the two.
        assert_eq!(
            cargo(&format!("{CARGO}\n[dependencies]\nlibc = \"0.2\"\n"))
                .unwrap_err()
                .code(),
            "malformed"
        );
    }

    #[test]
    fn only_a_cargo_manifest_is_read_as_one() {
        assert_eq!(
            cargo_added("crates/cart/Cargo.lock", CARGO, CARGO),
            Err(ManifestError::NotAManifest {
                path: "crates/cart/Cargo.lock".to_string()
            })
        );
        assert_eq!(
            cargo_added("crates\\cart\\Cargo.toml", CARGO, CARGO),
            Ok(vec![])
        );
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find type `ManifestError` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of reading `Cargo.toml`**

Add this to `crates/factory-presets/src/manifest.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/manifest.rs -->
```rust
//! Reading a dependency manifest before and after a change, to answer one question: did the
//! change only add dependencies?
//!
//! The two versions are parsed and compared as data, never as text, so reformatting or a
//! comment is not a change, and nothing can hide in formatting. Everything outside the
//! dependency tables must be equal: scripts, test targets, features, a build script.

use crate::types::{ManifestError, Side};
use std::collections::BTreeSet;

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

// ---------------------------------------------------------------------------------- cargo

/// Keys that say an added Cargo dependency does not come from crates.io by name.
const CARGO_FOREIGN_SOURCE: [&str; 4] = ["path", "git", "registry", "registry-index"];

fn parse_toml(text: &str, side: Side) -> Result<toml::Table, ManifestError> {
    strip_bom(text)
        .parse::<toml::Table>()
        .map_err(|e| ManifestError::Malformed {
            side,
            detail: e.to_string(),
        })
}

/// Remove and return the dependency table `key` of `table`; absent means empty.
fn take_toml_table(
    table: &mut toml::Table,
    key: &str,
    side: Side,
) -> Result<toml::Table, ManifestError> {
    match table.remove(key) {
        None => Ok(toml::Table::new()),
        Some(toml::Value::Table(inner)) => Ok(inner),
        Some(_) => Err(ManifestError::Malformed {
            side,
            detail: format!("{key} is not a table"),
        }),
    }
}

/// Remove and return `[workspace.dependencies]`.
fn take_workspace_dependencies(
    table: &mut toml::Table,
    side: Side,
) -> Result<toml::Table, ManifestError> {
    match table.get_mut("workspace") {
        Some(toml::Value::Table(workspace)) => take_toml_table(workspace, "dependencies", side),
        _ => Ok(toml::Table::new()),
    }
}

fn diff_cargo_dependencies(
    before: &toml::Table,
    after: &toml::Table,
    added: &mut BTreeSet<String>,
) -> Result<(), ManifestError> {
    for (name, requirement) in before {
        match after.get(name) {
            None => return Err(ManifestError::DependencyRemoved { name: name.clone() }),
            Some(now) if now != requirement => {
                return Err(ManifestError::DependencyChanged { name: name.clone() })
            }
            Some(_) => {}
        }
    }
    for (name, requirement) in after {
        if before.contains_key(name) {
            continue;
        }
        let mut package = name.clone();
        if let toml::Value::Table(detail) = requirement {
            if CARGO_FOREIGN_SOURCE
                .iter()
                .any(|key| detail.contains_key(*key))
            {
                return Err(ManifestError::UnsupportedSource { name: name.clone() });
            }
            if let Some(toml::Value::String(real)) = detail.get("package") {
                package = real.clone();
            }
        }
        added.insert(package);
    }
    Ok(())
}

/// The dotted key of the first place two TOML tables differ.
fn first_toml_difference(before: &toml::Table, after: &toml::Table, prefix: &str) -> String {
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    for key in keys {
        let here = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match (before.get(key), after.get(key)) {
            (Some(a), Some(b)) if a == b => {}
            (Some(toml::Value::Table(a)), Some(toml::Value::Table(b))) => {
                return first_toml_difference(a, b, &here)
            }
            _ => return here,
        }
    }
    prefix.to_string()
}

/// Dependencies a `Cargo.toml` gains: new entries in `[dependencies]`, `[dev-dependencies]`,
/// `[build-dependencies]` and `[workspace.dependencies]`. A renamed dependency
/// (`alias = { package = "real" }`) is reported by its real name.
pub(crate) fn cargo_added(
    path: &str,
    before: &str,
    after: &str,
) -> Result<Vec<String>, ManifestError> {
    if !file_name(path).eq_ignore_ascii_case("Cargo.toml") {
        return Err(ManifestError::NotAManifest {
            path: path.to_string(),
        });
    }
    let mut old = parse_toml(before, Side::Before)?;
    let mut new = parse_toml(after, Side::After)?;
    let mut added = BTreeSet::new();
    for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let was = take_toml_table(&mut old, key, Side::Before)?;
        let now = take_toml_table(&mut new, key, Side::After)?;
        diff_cargo_dependencies(&was, &now, &mut added)?;
    }
    let was = take_workspace_dependencies(&mut old, Side::Before)?;
    let now = take_workspace_dependencies(&mut new, Side::After)?;
    diff_cargo_dependencies(&was, &now, &mut added)?;
    if old != new {
        return Err(ManifestError::OtherChange {
            key: first_toml_difference(&old, &new, ""),
        });
    }
    Ok(added.into_iter().collect())
}
```

The two versions are compared as parsed tables, so a comment or a reflowed line is not a change. A table written twice is a TOML parse error, which is what we want: there is no "last one wins" to exploit.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 92 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/manifest.rs
git commit -m "feat(factory-presets): read what a Cargo.toml change added, and refuse anything else"
```

- [ ] **Step 6: Write the failing tests for reading `package.json`**

Add these tests to the test module of `crates/factory-presets/src/manifest.rs`, directly above the module's closing `}` (the last line of the file):

<!-- plan-op: into-tests crates/factory-presets/src/manifest.rs -->
```rust
    const PACKAGE: &str = r#"{
  "name": "cart",
  "version": "1.0.0",
  "type": "module",
  "scripts": { "test": "node --test" },
  "dependencies": { "decimal.js": "^10.4.3" },
  "devDependencies": { "vitest": "^3.0.0" }
}
"#;

    fn node(after: &str) -> Result<Vec<String>, ManifestError> {
        node_added("package.json", PACKAGE, after)
    }

    #[test]
    fn an_unchanged_or_reformatted_package_json_adds_nothing() {
        assert_eq!(node(PACKAGE), Ok(vec![]));
        let compact: serde_json::Value = serde_json::from_str(PACKAGE).unwrap();
        assert_eq!(node(&compact.to_string()), Ok(vec![]));
        assert_eq!(node(&PACKAGE.replace('\n', "\r\n")), Ok(vec![]));
        assert_eq!(node(&format!("\u{feff}{PACKAGE}")), Ok(vec![]));
    }

    #[test]
    fn added_node_dependencies_are_listed_sorted_from_both_tables() {
        let after = PACKAGE
            .replace(
                "\"decimal.js\": \"^10.4.3\"",
                "\"decimal.js\": \"^10.4.3\", \"zod\": \"^3.23.0\"",
            )
            .replace(
                "\"vitest\": \"^3.0.0\"",
                "\"vitest\": \"^3.0.0\", \"@types/node\": \"latest\"",
            );
        // A scoped name has a `/` in it; only the requirement is checked for one.
        assert_eq!(
            node(&after),
            Ok(vec!["@types/node".to_string(), "zod".to_string()])
        );
    }

    #[test]
    fn a_package_json_with_no_dependency_tables_can_gain_one() {
        let before = "{ \"name\": \"cart\" }";
        let after = "{ \"name\": \"cart\", \"dependencies\": { \"zod\": \"3.23.0\" } }";
        assert_eq!(
            node_added("package.json", before, after),
            Ok(vec!["zod".to_string()])
        );
    }

    #[test]
    fn an_added_node_dependency_that_is_not_a_registry_version_is_refused() {
        for spec in [
            "\"git+https://example.com/evil.git\"",
            "\"file:../evil\"",
            "\"npm:other-package@1.0.0\"",
            "\"https://example.com/evil.tgz\"",
            "\"someone/evil\"",
            "\"workspace:*\"",
            "{ \"version\": \"1\" }",
            "1",
        ] {
            let after = PACKAGE.replace(
                "\"decimal.js\": \"^10.4.3\"",
                &format!("\"decimal.js\": \"^10.4.3\", \"evil\": {spec}"),
            );
            assert_eq!(
                node(&after),
                Err(ManifestError::UnsupportedSource {
                    name: "evil".to_string()
                }),
                "{spec}"
            );
        }
    }

    #[test]
    fn removing_or_changing_a_node_dependency_is_refused() {
        assert_eq!(
            node(&PACKAGE.replace("\"vitest\": \"^3.0.0\"", "")),
            Err(ManifestError::DependencyRemoved {
                name: "vitest".to_string()
            })
        );
        assert_eq!(
            node(&PACKAGE.replace("^10.4.3", "^9.0.0")),
            Err(ManifestError::DependencyChanged {
                name: "decimal.js".to_string()
            })
        );
    }

    #[test]
    fn any_other_package_json_change_is_refused_and_named() {
        assert_eq!(
            node(&PACKAGE.replace("node --test", "node --test || true")),
            other_change("scripts.test")
        );
        assert_eq!(
            node(&PACKAGE.replace(
                "\"test\": \"node --test\"",
                "\"test\": \"node --test\", \"postinstall\": \"node steal.js\""
            )),
            other_change("scripts.postinstall")
        );
        assert_eq!(
            node(&PACKAGE.replace(
                "\"type\": \"module\",",
                "\"type\": \"module\", \"jest\": { \"testMatch\": [] },"
            )),
            other_change("jest")
        );
        assert_eq!(
            node(&PACKAGE.replace(
                "\"type\": \"module\",",
                "\"type\": \"module\", \"optionalDependencies\": { \"x\": \"1\" },"
            )),
            other_change("optionalDependencies")
        );
        assert_eq!(
            node(&PACKAGE.replace("\"type\": \"module\",", "")),
            other_change("type")
        );
    }

    #[test]
    fn a_package_json_that_does_not_parse_is_refused_with_its_side() {
        assert!(matches!(
            node("{ not json"),
            Err(ManifestError::Malformed {
                side: Side::After,
                ..
            })
        ));
        assert!(matches!(
            node_added("package.json", "", PACKAGE),
            Err(ManifestError::Malformed {
                side: Side::Before,
                ..
            })
        ));
        assert!(matches!(
            node("[]"),
            Err(ManifestError::Malformed {
                side: Side::After,
                ..
            })
        ));
        assert!(matches!(
            node(&PACKAGE.replace("{ \"vitest\": \"^3.0.0\" }", "\"none\"")),
            Err(ManifestError::Malformed {
                side: Side::After,
                ..
            })
        ));
    }

    #[test]
    fn only_a_package_json_is_read_as_one() {
        assert_eq!(
            node_added("package-lock.json", PACKAGE, PACKAGE),
            Err(ManifestError::NotAManifest {
                path: "package-lock.json".to_string()
            })
        );
        assert_eq!(
            node_added("packages\\web\\package.json", PACKAGE, PACKAGE),
            Ok(vec![])
        );
        assert_eq!(
            cargo_added("package.json", PACKAGE, PACKAGE)
                .unwrap_err()
                .code(),
            "not_a_manifest"
        );
    }
```

- [ ] **Step 7: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find function `node_added` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 8: Write the implementation of reading `package.json`**

Add this to `crates/factory-presets/src/manifest.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/manifest.rs -->
```rust
// ----------------------------------------------------------------------------------- node

type JsonObject = serde_json::Map<String, serde_json::Value>;

fn parse_json(text: &str, side: Side) -> Result<JsonObject, ManifestError> {
    match serde_json::from_str::<serde_json::Value>(strip_bom(text)) {
        Ok(serde_json::Value::Object(object)) => Ok(object),
        Ok(_) => Err(ManifestError::Malformed {
            side,
            detail: "the manifest is not a JSON object".to_string(),
        }),
        Err(e) => Err(ManifestError::Malformed {
            side,
            detail: e.to_string(),
        }),
    }
}

fn take_json_object(
    object: &mut JsonObject,
    key: &str,
    side: Side,
) -> Result<JsonObject, ManifestError> {
    match object.remove(key) {
        None => Ok(JsonObject::new()),
        Some(serde_json::Value::Object(inner)) => Ok(inner),
        Some(_) => Err(ManifestError::Malformed {
            side,
            detail: format!("{key} is not an object"),
        }),
    }
}

fn diff_node_dependencies(
    before: &JsonObject,
    after: &JsonObject,
    added: &mut BTreeSet<String>,
) -> Result<(), ManifestError> {
    for (name, requirement) in before {
        match after.get(name) {
            None => return Err(ManifestError::DependencyRemoved { name: name.clone() }),
            Some(now) if now != requirement => {
                return Err(ManifestError::DependencyChanged { name: name.clone() })
            }
            Some(_) => {}
        }
    }
    for (name, requirement) in after {
        if before.contains_key(name) {
            continue;
        }
        // A registry requirement is a version, a range or a tag. Anything with a `:` or a
        // `/` is a URL, a path, a git remote, a workspace link or an alias for another name.
        let from_registry = requirement
            .as_str()
            .is_some_and(|spec| !spec.contains(':') && !spec.contains('/'));
        if !from_registry {
            return Err(ManifestError::UnsupportedSource { name: name.clone() });
        }
        added.insert(name.clone());
    }
    Ok(())
}

/// The dotted key of the first place two JSON objects differ.
fn first_json_difference(before: &JsonObject, after: &JsonObject, prefix: &str) -> String {
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    for key in keys {
        let here = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match (before.get(key), after.get(key)) {
            (Some(a), Some(b)) if a == b => {}
            (Some(serde_json::Value::Object(a)), Some(serde_json::Value::Object(b))) => {
                return first_json_difference(a, b, &here)
            }
            _ => return here,
        }
    }
    prefix.to_string()
}

/// Dependencies a `package.json` gains: new entries in `dependencies` and `devDependencies`.
pub(crate) fn node_added(
    path: &str,
    before: &str,
    after: &str,
) -> Result<Vec<String>, ManifestError> {
    if !file_name(path).eq_ignore_ascii_case("package.json") {
        return Err(ManifestError::NotAManifest {
            path: path.to_string(),
        });
    }
    let mut old = parse_json(before, Side::Before)?;
    let mut new = parse_json(after, Side::After)?;
    let mut added = BTreeSet::new();
    for key in ["dependencies", "devDependencies"] {
        let was = take_json_object(&mut old, key, Side::Before)?;
        let now = take_json_object(&mut new, key, Side::After)?;
        diff_node_dependencies(&was, &now, &mut added)?;
    }
    if old != new {
        return Err(ManifestError::OtherChange {
            key: first_json_difference(&old, &new, ""),
        });
    }
    Ok(added.into_iter().collect())
}
```

A scoped package name such as `@types/node` has a `/` in it and is fine; it is the *requirement* that must not contain `:` or `/`. JSON has no duplicate-key error: `serde_json` keeps the last value, as `npm` does, so both sides read the same thing.

- [ ] **Step 9: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 100 passed; 0 failed`.

- [ ] **Step 10: Commit**

```bash
git add crates/factory-presets/src/manifest.rs
git commit -m "feat(factory-presets): read what a package.json change added, and refuse anything else"
```


### Task 20: The secret rules

**Files:**
- Create: `crates/factory-presets/src/secrets.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: nothing. The tests use the `regex` dev-dependency.
- Produces:
  - `pub struct SecretRule { pub id: &'static str, pub description: &'static str, pub pattern: &'static str }` — derives `Debug, Clone, Copy, PartialEq, Eq`; `pattern` is in `regex`-crate syntax
  - `pub fn secret_rules() -> &'static [SecretRule]` — ten rules, in a fixed order

**Write every example in parts, exactly as shown.** Do not "simplify" a `join(&[...])` into one string: a whole token-shaped string in this file, in a vector file or in a commit can make GitHub's push protection refuse the push (Review Focus R5). One of the tests below fails if this file matches any of its own rules.

- [ ] **Step 1: Write the failing tests for the rule list**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod cargo;
mod js_scan;
mod junit;
mod manifest;
mod marker;
mod node;
mod protected;
mod rust_scan;
mod secrets;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use secrets::{secret_rules, SecretRule};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/secrets.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/secrets.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    fn rule(id: &str) -> Regex {
        let rule = secret_rules()
            .iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("no rule {id}"));
        Regex::new(rule.pattern).unwrap()
    }

    /// Join the parts of an example. No example is written whole in this file, so that the
    /// file itself matches none of its own rules and no push-protection scanner stops it.
    fn join(parts: &[&str]) -> String {
        parts.concat()
    }

    #[test]
    fn every_rule_compiles_and_has_a_unique_kebab_case_id() {
        let rules = secret_rules();
        assert_eq!(rules.len(), 10);
        let mut ids: Vec<&str> = rules.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), rules.len());
        for rule in rules {
            assert!(
                Regex::new(rule.pattern).is_ok(),
                "{} does not compile",
                rule.id
            );
            assert!(!rule.description.is_empty());
            assert!(rule.id.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'));
        }
    }

    #[test]
    fn each_rule_finds_its_own_token_format() {
        let letters = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOP";
        let cases: Vec<(&str, String)> = vec![
            ("aws-access-key-id", join(&["AK", "IA", "ABCDEFGHIJKLMNOP"])),
            ("aws-access-key-id", join(&["AS", "IA", "0123456789ABCDEF"])),
            ("github-token", join(&["gh", "p_", &letters[..36]])),
            ("github-token", join(&["gh", "s_", &letters[..40]])),
            (
                "github-fine-grained-token",
                join(&["github", "_pat_", &letters[..22], "_", &letters[..20]]),
            ),
            (
                "anthropic-api-key",
                join(&["sk-", "ant-", "api03-", &letters[..30]]),
            ),
            ("openai-api-key", join(&["sk-", "proj-", &letters[..40]])),
            ("openai-api-key", join(&["sk-", &letters[..40]])),
            (
                "slack-token",
                join(&["xo", "xb-", "123456789012-", &letters[..12]]),
            ),
            ("stripe-live-key", join(&["sk_", "live_", &letters[..24]])),
            ("stripe-live-key", join(&["rk_", "live_", &letters[..24]])),
            ("google-api-key", join(&["AI", "za", &letters[..35]])),
            ("npm-token", join(&["np", "m_", &letters[..36]])),
            (
                "private-key-block",
                join(&["-----BEGIN ", "PRIVATE KEY-----"]),
            ),
            (
                "private-key-block",
                join(&["-----BEGIN RSA ", "PRIVATE KEY-----"]),
            ),
            (
                "private-key-block",
                join(&["-----BEGIN OPENSSH ", "PRIVATE KEY-----"]),
            ),
        ];
        for (id, example) in cases {
            let line = format!("const value = \"{example}\";");
            assert!(rule(id).is_match(&line), "{id} missed its example");
        }
    }

    #[test]
    fn rules_do_not_fire_on_look_alikes() {
        let letters = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOP";
        let cases: Vec<(&str, String)> = vec![
            // too short
            ("aws-access-key-id", join(&["AK", "IA", "ABCDEFGH"])),
            // part of a longer word
            (
                "aws-access-key-id",
                join(&["XAK", "IA", "ABCDEFGHIJKLMNOP"]),
            ),
            (
                "aws-access-key-id",
                join(&["AK", "IA", "ABCDEFGHIJKLMNOPQ"]),
            ),
            ("github-token", join(&["gh", "p_", "short"])),
            ("github-token", join(&["gh", "x_", &letters[..36]])),
            ("stripe-live-key", join(&["sk_", "test_", &letters[..24]])),
            ("stripe-live-key", join(&["pk_", "live_", &letters[..24]])),
            ("openai-api-key", "sk-short".to_string()),
            (
                "openai-api-key",
                "task-force-alpha-bravo-charlie-delta-echo".to_string(),
            ),
            ("npm-token", join(&["np", "m_", "short"])),
            ("slack-token", "xoxo-hugs".to_string()),
            (
                "private-key-block",
                "-----BEGIN PUBLIC KEY-----".to_string(),
            ),
            (
                "private-key-block",
                "-----BEGIN CERTIFICATE-----".to_string(),
            ),
            ("google-api-key", "AIzaShort".to_string()),
        ];
        for (id, text) in cases {
            assert!(!rule(id).is_match(&text), "{id} fired on {text:?}");
        }
    }

    #[test]
    fn this_source_file_matches_none_of_its_own_rules() {
        let source = include_str!("secrets.rs");
        for rule in secret_rules() {
            let found = Regex::new(rule.pattern).unwrap().find(source);
            assert!(found.is_none(), "{} matches this file: {found:?}", rule.id);
        }
    }

    #[test]
    fn a_rule_matches_inside_a_longer_line_and_across_many_lines() {
        let key = join(&["AK", "IA", "ABCDEFGHIJKLMNOP"]);
        let text = format!("line one\nexport AWS_KEY={key}\nline three\n");
        let found = rule("aws-access-key-id").find(&text).unwrap();
        assert_eq!(found.as_str(), key);
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved imports `secrets::secret_rules`, `secrets::SecretRule`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the rule list**

Add this to `crates/factory-presets/src/secrets.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/secrets.rs -->
```rust
//! The fixed list of secret-detection rules.
//!
//! A rule is data: an id, a description and a pattern. This crate compiles no pattern; the
//! caller does, with whatever engine it links. Patterns use the syntax of the `regex` crate
//! and only what that crate supports: no look-around and no back-references.
//!
//! Each rule matches a token format that a provider publishes and that is long and
//! distinctive enough not to occur by accident. There is no "looks like a password" rule: a
//! rule that fires on ordinary test fixtures stops units for nothing.

/// One secret-detection rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretRule {
    /// A stable name, kebab-case. A fingerprint of a finding includes it.
    pub id: &'static str,
    /// What the rule finds, for the person reading a report.
    pub description: &'static str,
    /// The pattern, in `regex`-crate syntax.
    pub pattern: &'static str,
}

const RULES: [SecretRule; 10] = [
    SecretRule {
        id: "aws-access-key-id",
        description: "An AWS access key id",
        pattern: r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
    },
    SecretRule {
        id: "github-token",
        description: "A GitHub personal, OAuth, user, server or refresh token",
        pattern: r"\bgh[pousr]_[A-Za-z0-9]{36,}\b",
    },
    SecretRule {
        id: "github-fine-grained-token",
        description: "A GitHub fine-grained personal access token",
        pattern: r"\bgithub_pat_[A-Za-z0-9_]{22,}\b",
    },
    SecretRule {
        id: "anthropic-api-key",
        description: "An Anthropic API key",
        pattern: r"\bsk-ant-[A-Za-z0-9_-]{24,}",
    },
    SecretRule {
        id: "openai-api-key",
        description: "An OpenAI API key",
        pattern: r"\bsk-(?:proj-|svcacct-)?[A-Za-z0-9]{32,}\b",
    },
    SecretRule {
        id: "slack-token",
        description: "A Slack bot, user, app or legacy token",
        pattern: r"\bxox[abprs]-[A-Za-z0-9-]{10,}",
    },
    SecretRule {
        id: "stripe-live-key",
        description: "A Stripe live secret or restricted key",
        pattern: r"\b[sr]k_live_[A-Za-z0-9]{24,}\b",
    },
    SecretRule {
        id: "google-api-key",
        description: "A Google API key",
        pattern: r"\bAIza[0-9A-Za-z_-]{35}",
    },
    SecretRule {
        id: "npm-token",
        description: "An npm access token",
        pattern: r"\bnpm_[A-Za-z0-9]{36}\b",
    },
    SecretRule {
        id: "private-key-block",
        description: "The first line of a PEM private key",
        pattern: r"-----BEGIN (?:[A-Z]+ )*PRIVATE KEY-----",
    },
];

/// Every secret-detection rule, in a fixed order.
pub fn secret_rules() -> &'static [SecretRule] {
    &RULES
}
```

There is no generic "password = ..." rule. Whether a new match is a finding is the caller's job (it compares against a baseline); this crate only says what a token looks like.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 105 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/secrets.rs
git commit -m "feat(factory-presets): the fixed secret-detection rules"
```


### Task 21: The two presets and the lookup

**Files:**
- Create: `crates/factory-presets/src/presets.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: `junit::{read_cases, RawCase}` (Task 12); `marker::criterion_of` (13); `protected::{cargo, node}` (14); `cargo::{enumerate_ids, test_regions}` (16); `node::{enumerate_ids, test_regions}` (18); `manifest::{cargo_added, node_added}` (19); the trait and types (11).
- Produces:
  - `pub struct CargoPreset;` and `pub struct NodePreset;` — unit structs deriving `Debug, Clone, Copy, Default`, each `impl Preset`
  - `pub fn preset(name: &str) -> Option<&'static dyn Preset>` — `"cargo"` or `"node"`, exact match

Assumptions A1, A5, A6 and A7 live in `cargo_id` and `node_id` here. The three report constants in the tests are the hand-written, assumed shapes; the test `a_node_test_report_without_file_attributes_cannot_tell_two_files_apart` is the one built from an observed report.

- [ ] **Step 1: Write the failing tests for the presets**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod cargo;
mod js_scan;
mod junit;
mod manifest;
mod marker;
mod node;
mod presets;
mod protected;
mod rust_scan;
mod secrets;
mod types;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use presets::{preset, CargoPreset, NodePreset};
pub use secrets::{secret_rules, SecretRule};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/presets.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/presets.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TestStatus;

    /// ASSUMED shape of a cargo-nextest report (see the module comment).
    const NEXTEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="1" errors="0">
    <testsuite name="cart" tests="2" disabled="0" errors="0" failures="1">
        <testcase name="discount::tests::ac1_ten_percent_off" classname="cart" time="0.004">
        </testcase>
        <testcase name="discount::tests::ac2_rejects_more_than_all" classname="cart" time="0.005">
            <failure type="test failure">thread panicked at src/discount.rs:19:9</failure>
        </testcase>
    </testsuite>
    <testsuite name="cart::checkout" tests="1" disabled="0" errors="0" failures="0">
        <testcase name="ac3_checkout_totals" classname="cart::checkout" time="0.011">
        </testcase>
    </testsuite>
</testsuites>
"#;

    /// ASSUMED shape of a vitest report.
    const VITEST: &str = r#"<?xml version="1.0" encoding="UTF-8" ?>
<testsuites name="vitest tests" tests="2" failures="0" errors="0" time="0.5">
    <testsuite name="src/cart.test.ts" tests="2" failures="0" errors="0" skipped="1" time="0.01">
        <testcase classname="src/cart.test.ts" name="cart &gt; ac1 applies a percent code" time="0.002">
        </testcase>
        <testcase classname="src/cart.test.ts" name="cart &gt; unknown codes &gt; ac2 leaves the total unchanged" time="0">
            <skipped/>
        </testcase>
    </testsuite>
</testsuites>
"#;

    /// ASSUMED shape of a `node --test` report.
    const NODE_TEST: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<testsuites>
	<testsuite name="cart" time="0.002" disabled="0" errors="0" tests="2" failures="1" skipped="0" hostname="host">
		<testcase name="ac1 applies a percent code" time="0.001" classname="test" file="test/cart.test.js"/>
		<testsuite name="unknown codes" time="0.001" disabled="0" errors="0" tests="1" failures="1" skipped="0" hostname="host">
			<testcase name="ac2 leaves the total unchanged" time="0.001" classname="test" file="test/cart.test.js" failure="9000 !== 10000">
				<failure type="testCodeFailure" message="9000 !== 10000">stack</failure>
			</testcase>
		</testsuite>
	</testsuite>
	<testcase name="top level" time="0.0005" classname="test" file="test\cart.test.js"/>
	<!-- tests 3 -->
</testsuites>
"#;

    fn case(id: &str, status: TestStatus) -> TestCase {
        TestCase {
            id: id.to_string(),
            status,
        }
    }

    #[test]
    fn lookup_is_by_exact_name() {
        assert_eq!(preset("cargo").unwrap().name(), "cargo");
        assert_eq!(preset("node").unwrap().name(), "node");
        assert!(preset("Cargo").is_none());
        assert!(preset("python").is_none());
        assert!(preset("").is_none());
    }

    #[test]
    fn a_preset_reference_can_cross_threads() {
        fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
        assert_send_sync(preset("cargo").unwrap());
    }

    #[test]
    fn cargo_ids_are_classname_then_name() {
        assert_eq!(
            preset("cargo").unwrap().read_report(NEXTEST).unwrap(),
            vec![
                case(
                    "cart > discount::tests::ac1_ten_percent_off",
                    TestStatus::Passed
                ),
                case(
                    "cart > discount::tests::ac2_rejects_more_than_all",
                    TestStatus::Failed
                ),
                case("cart::checkout > ac3_checkout_totals", TestStatus::Passed),
            ]
        );
    }

    #[test]
    fn vitest_ids_are_the_file_then_the_reported_name() {
        assert_eq!(
            preset("node").unwrap().read_report(VITEST).unwrap(),
            vec![
                case(
                    "src/cart.test.ts > cart > ac1 applies a percent code",
                    TestStatus::Passed
                ),
                case(
                    "src/cart.test.ts > cart > unknown codes > ac2 leaves the total unchanged",
                    TestStatus::Skipped
                ),
            ]
        );
    }

    #[test]
    fn node_test_ids_are_the_file_then_the_suites_then_the_name() {
        assert_eq!(
            preset("node").unwrap().read_report(NODE_TEST).unwrap(),
            vec![
                case(
                    "test/cart.test.js > cart > ac1 applies a percent code",
                    TestStatus::Passed
                ),
                case(
                    "test/cart.test.js > cart > unknown codes > ac2 leaves the total unchanged",
                    TestStatus::Failed
                ),
                case("test/cart.test.js > top level", TestStatus::Passed),
            ]
        );
    }

    #[test]
    fn an_id_reported_twice_is_an_error_not_two_results() {
        let twice = NEXTEST.replace("ac2_rejects_more_than_all", "ac1_ten_percent_off");
        assert_eq!(
            preset("cargo").unwrap().read_report(&twice),
            Err(ReportError::DuplicateId {
                id: "cart > discount::tests::ac1_ten_percent_off".to_string()
            })
        );
        let twice = r#"<testsuites>
            <testcase name="top level" classname="test" file="test/a.test.js"/>
            <testcase name="top level" classname="test" file="test/a.test.js"/>
        </testsuites>"#;
        assert_eq!(
            preset("node").unwrap().read_report(twice),
            Err(ReportError::DuplicateId {
                id: "test/a.test.js > top level".to_string()
            })
        );
    }

    /// OBSERVED, not assumed: this is what `node --test --test-reporter=junit` wrote on
    /// Node 22.17.1 for two test files that each declare `cart > ac1 applies a percent code`.
    /// There is no `file` attribute, so nothing tells the two files apart. The reader refuses
    /// the report; it does not guess.
    #[test]
    fn a_node_test_report_without_file_attributes_cannot_tell_two_files_apart() {
        let observed = r#"<?xml version="1.0" encoding="utf-8"?>
<testsuites>
	<testsuite name="cart" time="0.001688" disabled="0" errors="0" tests="1" failures="0" skipped="0" hostname="check">
		<testcase name="ac1 applies a percent code" time="0.000571" classname="test"/>
	</testsuite>
	<testsuite name="cart" time="0.001473" disabled="0" errors="0" tests="1" failures="0" skipped="0" hostname="check">
		<testcase name="ac1 applies a percent code" time="0.000547" classname="test"/>
	</testsuite>
	<!-- tests 2 -->
</testsuites>
"#;
        assert_eq!(
            preset("node").unwrap().read_report(observed),
            Err(ReportError::DuplicateId {
                id: "test > cart > ac1 applies a percent code".to_string()
            })
        );
    }

    #[test]
    fn the_same_test_name_in_two_classes_is_two_ids() {
        let xml = r#"<testsuites>
            <testsuite name="a"><testcase name="tests::works" classname="a"/></testsuite>
            <testsuite name="b"><testcase name="tests::works" classname="b"/></testsuite>
        </testsuites>"#;
        assert_eq!(preset("cargo").unwrap().read_report(xml).unwrap().len(), 2);
    }

    #[test]
    fn a_case_without_the_attribute_its_id_needs_is_refused() {
        let xml = r#"<testsuites><testcase name="t"/></testsuites>"#;
        for name in ["cargo", "node"] {
            assert_eq!(
                preset(name).unwrap().read_report(xml),
                Err(ReportError::MissingAttribute {
                    attribute: "classname"
                })
            );
        }
    }

    #[test]
    fn report_errors_pass_through_both_presets() {
        for name in ["cargo", "node"] {
            let p = preset(name).unwrap();
            assert_eq!(p.read_report(""), Err(ReportError::Empty));
            assert_eq!(p.read_report("<html/>").unwrap_err().code(), "not_junit");
            assert_eq!(p.read_report("<testsuites/>"), Ok(vec![]));
        }
    }

    #[test]
    fn each_method_reaches_its_presets_own_rules() {
        let cargo = preset("cargo").unwrap();
        let node = preset("node").unwrap();
        assert!(cargo.is_protected("build.rs") && !node.is_protected("build.rs"));
        assert!(node.is_protected(".npmrc") && !cargo.is_protected(".npmrc"));
        assert_eq!(
            cargo.criterion_of("cart > tests::ac1_x"),
            Some("AC-1".to_string())
        );
        assert_eq!(
            node.criterion_of("a.test.js > ac2 y"),
            Some("AC-2".to_string())
        );
        assert_eq!(
            cargo.enumerate_ids("crates/cart/src/lib.rs", "#[test]\nfn t() {}\n"),
            Ok(vec!["cart > t".to_string()])
        );
        assert_eq!(
            node.enumerate_ids("a.test.js", "it('t', () => {})"),
            Ok(vec!["a.test.js > t".to_string()])
        );
        assert_eq!(
            cargo.test_regions("src/lib.rs", "#[cfg(test)]\nmod t {}\n"),
            vec![0..21]
        );
        assert_eq!(
            node.test_regions("a.ts", "if (import.meta.vitest) {}"),
            vec![0..26]
        );
        assert_eq!(
            cargo
                .added_dependencies("package.json", "{}", "{}")
                .unwrap_err()
                .code(),
            "not_a_manifest"
        );
        assert_eq!(
            node.added_dependencies("package.json", "{}", "{}"),
            Ok(vec![])
        );
    }

    /// The property spike S6 exists to confirm, stated on the assumed fixtures: what the
    /// preset enumerates from source is exactly what the runner reports.
    #[test]
    fn enumerated_ids_equal_reported_ids_on_the_assumed_fixtures() {
        let cargo = preset("cargo").unwrap();
        let unit = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn ac1_ten_percent_off() {}\n    #[test]\n    fn ac2_rejects_more_than_all() {}\n}\n";
        let integration = "#[test]\nfn ac3_checkout_totals() {}\n";
        let mut enumerated = cargo
            .enumerate_ids("crates/cart/src/discount.rs", unit)
            .unwrap();
        enumerated.extend(
            cargo
                .enumerate_ids("crates/cart/tests/checkout.rs", integration)
                .unwrap(),
        );
        let reported: Vec<String> = cargo
            .read_report(NEXTEST)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(enumerated, reported);

        let node = preset("node").unwrap();
        let vitest_source = "describe('cart', () => {\n  it('ac1 applies a percent code', () => {})\n  describe('unknown codes', () => {\n    it.skip('ac2 leaves the total unchanged', () => {})\n  })\n})\n";
        let reported: Vec<String> = node
            .read_report(VITEST)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(
            node.enumerate_ids("src/cart.test.ts", vitest_source)
                .unwrap(),
            reported
        );

        let node_source =
            format!("{vitest_source}it('top level', () => {{}})\n").replace("it.skip", "it");
        let reported: Vec<String> = node
            .read_report(NODE_TEST)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(
            node.enumerate_ids("test/cart.test.js", &node_source)
                .unwrap(),
            reported
        );
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0432]: unresolved imports `presets::preset`, `presets::CargoPreset`, `presets::NodePreset`
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the presets**

Add this to `crates/factory-presets/src/presets.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/presets.rs -->
```rust
//! The two presets, wired to the modules that do their work, and the lookup by name.
//!
//! This is also where a raw report case becomes a test id.
//!
//! ASSUMPTION (spike S6 has not confirmed it), one rule per preset:
//!
//! - `cargo` (cargo-nextest): the id is the case's `classname`, then its `name`.
//! - `node` (vitest, `node --test`): the class is the case's `file` attribute when it has
//!   one, otherwise its `classname`. The name is the names of the enclosing `<testsuite>`
//!   elements, then the case's `name`, where a suite named exactly like the class or the
//!   classname is left out (vitest wraps each file in a suite named after the file).

use crate::junit::{read_cases, RawCase};
use crate::types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, TestCase, ID_SEPARATOR,
};
use crate::{cargo, manifest, marker, node, protected};
use std::collections::BTreeSet;
use std::ops::Range;

/// Turn raw cases into test cases with `id_of`, refusing a repeated id.
fn cases_with(
    junit_xml: &str,
    id_of: fn(&RawCase) -> Result<String, ReportError>,
) -> Result<Vec<TestCase>, ReportError> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cases = Vec::new();
    for raw in read_cases(junit_xml)? {
        let id = id_of(&raw)?;
        if !seen.insert(id.clone()) {
            return Err(ReportError::DuplicateId { id });
        }
        cases.push(TestCase {
            id,
            status: raw.status,
        });
    }
    Ok(cases)
}

fn cargo_id(raw: &RawCase) -> Result<String, ReportError> {
    let class = raw
        .classname
        .as_deref()
        .ok_or(ReportError::MissingAttribute {
            attribute: "classname",
        })?;
    Ok(test_id(class, &raw.name))
}

fn node_id(raw: &RawCase) -> Result<String, ReportError> {
    let class = raw
        .file
        .as_deref()
        .or(raw.classname.as_deref())
        .ok_or(ReportError::MissingAttribute {
            attribute: "classname",
        })?
        .replace('\\', "/");
    let mut parts: Vec<&str> = raw
        .suites
        .iter()
        .map(String::as_str)
        .filter(|suite| *suite != class && Some(*suite) != raw.classname.as_deref())
        .collect();
    parts.push(&raw.name);
    Ok(test_id(&class, &parts.join(ID_SEPARATOR)))
}

/// The `cargo` preset: Rust, tested with cargo-nextest.
#[derive(Debug, Clone, Copy, Default)]
pub struct CargoPreset;

impl Preset for CargoPreset {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn read_report(&self, junit_xml: &str) -> Result<Vec<TestCase>, ReportError> {
        cases_with(junit_xml, cargo_id)
    }

    fn enumerate_ids(&self, path: &str, source: &str) -> Result<Vec<String>, EnumerateError> {
        cargo::enumerate_ids(path, source)
    }

    fn criterion_of(&self, test_id: &str) -> Option<String> {
        marker::criterion_of(test_id)
    }

    fn is_protected(&self, path: &str) -> bool {
        protected::cargo(path)
    }

    fn test_regions(&self, path: &str, source: &str) -> Vec<Range<usize>> {
        cargo::test_regions(path, source)
    }

    fn added_dependencies(
        &self,
        path: &str,
        before: &str,
        after: &str,
    ) -> Result<Vec<String>, ManifestError> {
        manifest::cargo_added(path, before, after)
    }
}

/// The `node` preset: JavaScript and TypeScript, tested with vitest or `node --test`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodePreset;

impl Preset for NodePreset {
    fn name(&self) -> &'static str {
        "node"
    }

    fn read_report(&self, junit_xml: &str) -> Result<Vec<TestCase>, ReportError> {
        cases_with(junit_xml, node_id)
    }

    fn enumerate_ids(&self, path: &str, source: &str) -> Result<Vec<String>, EnumerateError> {
        node::enumerate_ids(path, source)
    }

    fn criterion_of(&self, test_id: &str) -> Option<String> {
        marker::criterion_of(test_id)
    }

    fn is_protected(&self, path: &str) -> bool {
        protected::node(path)
    }

    fn test_regions(&self, path: &str, source: &str) -> Vec<Range<usize>> {
        node::test_regions(path, source)
    }

    fn added_dependencies(
        &self,
        path: &str,
        before: &str,
        after: &str,
    ) -> Result<Vec<String>, ManifestError> {
        manifest::node_added(path, before, after)
    }
}

static CARGO: CargoPreset = CargoPreset;
static NODE: NodePreset = NodePreset;

/// The preset called `name` (`"cargo"` or `"node"`), or `None`. Names are exact: a
/// repository on a stack with no preset is refused by the caller, not guessed at here.
pub fn preset(name: &str) -> Option<&'static dyn Preset> {
    match name {
        "cargo" => Some(&CARGO),
        "node" => Some(&NODE),
        _ => None,
    }
}
```

`enumerated_ids_equal_reported_ids_on_the_assumed_fixtures` is the property spike S6 exists to confirm, stated on assumed data. It proves the two halves of this crate agree with each other; it does not prove either agrees with a real runner.

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 117 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/presets.rs
git commit -m "feat(factory-presets): CargoPreset, NodePreset and preset(name)"
```


### Task 22: The testkit's builders

**Files:**
- Create: `crates/factory-presets/src/testkit.rs` (tests inline)
- Modify: `crates/factory-presets/src/lib.rs`

**Interfaces:**
- Consumes: `test_id`, `TestCase`, `TestStatus`, `ID_SEPARATOR`; `preset` in tests.
- Produces, in `pub mod testkit`, compiled under `cfg(test)` or the `testkit` feature:
  - `pub fn xml_escape(text: &str) -> String`
  - `pub fn passed(ids: &[&str]) -> Vec<TestCase>`
  - `pub struct ReportBuilder` with `fn nextest() -> Self`, `fn vitest() -> Self`, `fn node_test() -> Self`, `fn case(self, class: &str, name: &str, status: TestStatus) -> Self`, `fn expected(&self) -> Vec<TestCase>` and `fn build(&self) -> String`

`ReportBuilder` writes the assumed shapes (A1, A5, A6, A7). Task 25 changes it together with the mapping functions if the spike says a shape is different.

- [ ] **Step 1: Write the failing tests for the builders**

Make `crates/factory-presets/src/lib.rs` read exactly:

<!-- plan-op: create crates/factory-presets/src/lib.rs -->
```rust
//! `factory-presets`: the rules that differ from one language stack to the next and that a
//! repository must not be able to configure away. For `cargo` and for `node`: how to read a
//! test report into test ids and outcomes, how to enumerate test ids from source, how a
//! criterion marker appears in a test name, which paths are protected, which bytes of a
//! source file are test code, how to read a dependency manifest, and the fixed
//! secret-detection rules.
//!
//! Nothing here touches a file, a socket, a process or a clock: callers hand in text and get
//! plain values back. The control plane and the harness both link this crate, so a bug in it
//! misleads both of them in the same way.

mod cargo;
mod js_scan;
mod junit;
mod manifest;
mod marker;
mod node;
mod presets;
mod protected;
mod rust_scan;
mod secrets;
mod types;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use junit::{MAX_REPORT_BYTES, MAX_REPORT_DEPTH};
pub use presets::{preset, CargoPreset, NodePreset};
pub use secrets::{secret_rules, SecretRule};
pub use types::{
    test_id, EnumerateError, ManifestError, Preset, ReportError, Side, TestCase, TestStatus,
    ID_SEPARATOR,
};

/// The version of the rules in this crate. The control plane and the harness each announce
/// the value they were built with when a session opens, and refuse to work together if the
/// two differ.
pub const PRESETS_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_version_is_a_semver_triple() {
        let parts: Vec<&str> = PRESETS_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

Create `crates/factory-presets/src/testkit.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/testkit.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset;

    #[test]
    fn xml_escape_covers_the_five_special_characters() {
        assert_eq!(
            xml_escape("a < b && c > \"d\" 'e'"),
            "a &lt; b &amp;&amp; c &gt; &quot;d&quot; &apos;e&apos;"
        );
        assert_eq!(xml_escape("plain \u{e9}"), "plain \u{e9}");
    }

    #[test]
    fn passed_marks_every_id_passed() {
        assert_eq!(
            passed(&["a > b", "a > c"]),
            vec![
                TestCase {
                    id: "a > b".to_string(),
                    status: TestStatus::Passed
                },
                TestCase {
                    id: "a > c".to_string(),
                    status: TestStatus::Passed
                },
            ]
        );
        assert!(passed(&[]).is_empty());
    }

    #[test]
    fn a_nextest_report_reads_back_through_the_cargo_preset() {
        let report = ReportBuilder::nextest()
            .case(
                "cart",
                "discount::tests::ac1_ten_percent_off",
                TestStatus::Passed,
            )
            .case("cart", "discount::tests::ac2_rejects", TestStatus::Failed)
            .case("cart::checkout", "ac3_totals", TestStatus::Errored)
            .case("cart::checkout", "ac4_later", TestStatus::Skipped);
        assert_eq!(
            preset("cargo").unwrap().read_report(&report.build()),
            Ok(report.expected())
        );
    }

    #[test]
    fn vitest_and_node_test_reports_read_back_through_the_node_preset() {
        for builder in [ReportBuilder::vitest(), ReportBuilder::node_test()] {
            let report = builder
                .case(
                    "src/cart.test.ts",
                    "cart > ac1 applies a code",
                    TestStatus::Passed,
                )
                .case(
                    "src/cart.test.ts",
                    "cart > unknown > ac2 rejects",
                    TestStatus::Failed,
                )
                .case("src/cart.test.ts", "top level", TestStatus::Skipped);
            assert_eq!(
                preset("node").unwrap().read_report(&report.build()),
                Ok(report.expected())
            );
        }
    }

    #[test]
    fn names_with_xml_special_characters_survive_the_round_trip() {
        let name = "cart > ac1 <10% & \"quoted\" 'single' \u{2713}";
        for (preset_name, builder) in [
            ("cargo", ReportBuilder::nextest()),
            ("node", ReportBuilder::vitest()),
            ("node", ReportBuilder::node_test()),
        ] {
            let report = builder.case("src/a&b.test.ts", name, TestStatus::Passed);
            let cases = preset(preset_name)
                .unwrap()
                .read_report(&report.build())
                .unwrap();
            assert_eq!(cases, report.expected(), "{preset_name}");
            assert_eq!(cases[0].id, format!("src/a&b.test.ts > {name}"));
        }
    }

    #[test]
    fn an_empty_builder_is_a_report_with_no_cases() {
        let report = ReportBuilder::vitest();
        assert_eq!(
            preset("node").unwrap().read_report(&report.build()),
            Ok(vec![])
        );
        assert!(report.expected().is_empty());
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0422]: cannot find struct, variant or union type `TestCase` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 3: Write the implementation of the builders**

Add this to `crates/factory-presets/src/testkit.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/testkit.rs -->
```rust
//! Builders for this crate's types, and the shared test vectors.
//!
//! Compiled for this crate's own tests, and for other crates behind the `testkit` feature.
//! [`ReportBuilder`] writes a JUnit report in one of the three shapes this crate reads, so a
//! consumer's test can say "these tests passed, this one failed" without writing XML.

use crate::types::{test_id, TestCase, TestStatus, ID_SEPARATOR};

/// Escape text for use in an XML attribute value or element body.
pub fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

/// `ids`, each as a passed test case.
pub fn passed(ids: &[&str]) -> Vec<TestCase> {
    ids.iter()
        .map(|id| TestCase {
            id: id.to_string(),
            status: TestStatus::Passed,
        })
        .collect()
}

/// Which runner's report shape to write. These are the ASSUMED shapes (spike S6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Nextest,
    Vitest,
    NodeTest,
}

/// Builds a JUnit XML report. Each case is given by the class and name its test id is made
/// of, so `build()` and `expected()` always agree.
#[derive(Debug, Clone)]
pub struct ReportBuilder {
    shape: Shape,
    cases: Vec<(String, String, TestStatus)>,
}

impl ReportBuilder {
    /// A cargo-nextest report, read by the `cargo` preset.
    pub fn nextest() -> Self {
        Self::new(Shape::Nextest)
    }

    /// A vitest report, read by the `node` preset.
    pub fn vitest() -> Self {
        Self::new(Shape::Vitest)
    }

    /// A `node --test` report, read by the `node` preset.
    pub fn node_test() -> Self {
        Self::new(Shape::NodeTest)
    }

    fn new(shape: Shape) -> Self {
        ReportBuilder {
            shape,
            cases: Vec::new(),
        }
    }

    /// Add a test. `class` is the binary id (nextest) or the file path (vitest, node);
    /// `name` is the rest of the id, with nested groups joined by `" > "`.
    pub fn case(mut self, class: &str, name: &str, status: TestStatus) -> Self {
        self.cases
            .push((class.to_string(), name.to_string(), status));
        self
    }

    /// The test cases a preset should read back from [`ReportBuilder::build`].
    pub fn expected(&self) -> Vec<TestCase> {
        self.cases
            .iter()
            .map(|(class, name, status)| TestCase {
                id: test_id(class, name),
                status: *status,
            })
            .collect()
    }

    /// The report's XML.
    pub fn build(&self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites>\n");
        for (class, name, status) in &self.cases {
            let class = xml_escape(class);
            let outcome = match status {
                TestStatus::Passed => "",
                TestStatus::Failed => "<failure message=\"failed\">details</failure>",
                TestStatus::Errored => "<error message=\"errored\">details</error>",
                TestStatus::Skipped => "<skipped/>",
            };
            match self.shape {
                Shape::Nextest | Shape::Vitest => {
                    xml.push_str(&format!(
                        "<testsuite name=\"{class}\"><testcase name=\"{}\" classname=\"{class}\">{outcome}</testcase></testsuite>\n",
                        xml_escape(name)
                    ));
                }
                Shape::NodeTest => {
                    let mut parts: Vec<&str> = name.split(ID_SEPARATOR).collect();
                    let leaf = parts.pop().unwrap_or("");
                    for group in &parts {
                        xml.push_str(&format!("<testsuite name=\"{}\">", xml_escape(group)));
                    }
                    xml.push_str(&format!(
                        "<testcase name=\"{}\" classname=\"test\" file=\"{class}\">{outcome}</testcase>",
                        xml_escape(leaf)
                    ));
                    for _ in &parts {
                        xml.push_str("</testsuite>");
                    }
                    xml.push('\n');
                }
            }
        }
        xml.push_str("</testsuites>\n");
        xml
    }
}
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 123 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/factory-presets/src/lib.rs crates/factory-presets/src/testkit.rs
git commit -m "feat(factory-presets): testkit with a JUnit report builder"
```


### Task 23: The shared vectors

**Files:**
- Create: `crates/factory-presets/vectors/.gitattributes`, `crates/factory-presets/vectors/index.json`, four files under `crates/factory-presets/vectors/reports/`, seven under `crates/factory-presets/vectors/sources/`, eight under `crates/factory-presets/vectors/manifests/`
- Create: `crates/factory-presets/src/testkit/vectors.rs` (tests inline)
- Modify: `crates/factory-presets/src/testkit.rs` (one line)

**Interfaces:**
- Consumes: `preset`, `secret_rules`, `Preset`, `TestCase`, `TestStatus`.
- Produces, in `pub mod testkit::vectors`:
  - `pub const INDEX: &str` — `vectors/index.json`
  - `pub const FILES: &[(&str, &str)]` — every data file, by its path under `vectors/`
  - `pub fn run_all() -> Result<usize, Vec<String>>` — every vector except the secret rules
  - `pub fn run_secrets(is_match: impl Fn(&str, &str) -> bool) -> Result<usize, Vec<String>>` — the secret-rule vectors, with the caller's pattern engine

**The three reports and the four test sources they pair with are ASSUMED shapes** (table at the top of this lane). `index.json`'s `pairs` say, for each report, which sources produced it; the runner checks both that the report reads to the listed cases and that the ids enumerated from the sources are exactly the ids in the report.

The region offsets in `index.json` (`[109, 604]`, `[18, 97]`, `[70, 201]`) are byte offsets into the source files as written here with LF endings. `vectors/.gitattributes` keeps them LF on every checkout.

A second repository runs the same vectors with two tests:

```rust
// in the consumer's Cargo.toml: factory-presets = { ..., features = ["testkit"] } and regex = "1", both under [dev-dependencies]
#[test]
fn factory_presets_vectors_pass() {
    assert_eq!(factory_presets::testkit::vectors::run_all(), Ok(51));
}

#[test]
fn factory_presets_secret_vectors_pass() {
    let is_match = |pattern: &str, text: &str| regex::Regex::new(pattern).unwrap().is_match(text);
    assert_eq!(factory_presets::testkit::vectors::run_secrets(is_match), Ok(15));
}
```

- [ ] **Step 1: Write the vector data**

Twenty-one files. Write each exactly as shown, with LF line endings and one newline at the end. `reports/node-test.xml` is indented with tab characters, as the runner writes it. The secret examples in `index.json` are stored in parts on purpose (Review Focus R5): keep them in parts.

Create `crates/factory-presets/vectors/.gitattributes` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/.gitattributes -->
```text
* text eol=lf
```

Create `crates/factory-presets/vectors/reports/nextest.xml` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/reports/nextest.xml -->
```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="4" failures="1" errors="0">
    <testsuite name="cart" tests="3" disabled="0" errors="0" failures="1">
        <testcase name="discount::tests::ac1_ten_percent_off" classname="cart" time="0.004">
        </testcase>
        <testcase name="discount::tests::ac2_rejects_more_than_all" classname="cart" time="0.005">
            <failure type="test failure">thread 'discount::tests::ac2_rejects_more_than_all' panicked at crates/cart/src/discount.rs:19:9</failure>
            <system-err>attempt to subtract with overflow</system-err>
        </testcase>
        <testcase name="discount::tests::rounding::ac3_rounds_down" classname="cart" time="0.004">
        </testcase>
    </testsuite>
    <testsuite name="cart::checkout" tests="1" disabled="0" errors="0" failures="0">
        <testcase name="ac4_checkout_totals" classname="cart::checkout" time="0.011">
        </testcase>
    </testsuite>
</testsuites>
```

Create `crates/factory-presets/vectors/reports/vitest.xml` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/reports/vitest.xml -->
```xml
<?xml version="1.0" encoding="UTF-8" ?>
<testsuites name="vitest tests" tests="4" failures="1" errors="0" time="0.503">
    <testsuite name="src/cart.test.ts" timestamp="2026-10-04T10:00:00.000Z" hostname="check" tests="4" failures="1" errors="0" skipped="1" time="0.013">
        <testcase classname="src/cart.test.ts" name="cart &gt; ac1 applies a percent code" time="0.002">
        </testcase>
        <testcase classname="src/cart.test.ts" name="cart &gt; unknown codes &gt; ac2 leaves the total unchanged" time="0">
            <skipped/>
        </testcase>
        <testcase classname="src/cart.test.ts" name="cart &gt; totals are never negative (ac3)" time="0.004">
            <failure message="expected -1 to be greater than or equal to 0" type="AssertionError">
AssertionError: expected -1 to be greater than or equal to 0
 &gt; src/cart.test.ts:15:28
            </failure>
        </testcase>
        <testcase classname="src/cart.test.ts" name="top level &amp; &quot;quoted&quot;" time="0.001">
        </testcase>
    </testsuite>
</testsuites>
```

Create `crates/factory-presets/vectors/reports/node-test.xml` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/reports/node-test.xml -->
```xml
<?xml version="1.0" encoding="utf-8"?>
<testsuites>
	<testsuite name="cart" time="0.002125" disabled="0" errors="0" tests="2" failures="1" skipped="0" hostname="check">
		<testcase name="ac1 applies a percent code" time="0.000913" classname="test" file="test/cart.test.js"/>
		<testsuite name="unknown codes" time="0.000712" disabled="0" errors="0" tests="1" failures="1" skipped="0" hostname="check">
			<testcase name="ac2 leaves the total unchanged" time="0.000611" classname="test" file="test/cart.test.js" failure="9000 !== 10000">
				<failure type="testCodeFailure" message="9000 !== 10000">
[Error [ERR_TEST_FAILURE]: 9000 !== 10000]
				</failure>
			</testcase>
		</testsuite>
	</testsuite>
	<testcase name="top level" time="0.000101" classname="test" file="test/cart.test.js">
		<skipped type="todo" message="later"/>
	</testcase>
	<!-- tests 3 -->
	<!-- pass 1 -->
</testsuites>
```

Create `crates/factory-presets/vectors/reports/duplicate.xml` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/reports/duplicate.xml -->
```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="2" failures="0" errors="0">
    <testsuite name="cart" tests="2" disabled="0" errors="0" failures="0">
        <testcase name="discount::tests::ac1_ten_percent_off" classname="cart" time="0.004">
        </testcase>
        <testcase name="discount::tests::ac1_ten_percent_off" classname="cart" time="0.004">
        </testcase>
    </testsuite>
</testsuites>
```

Create `crates/factory-presets/vectors/sources/discount.rs` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/sources/discount.rs -->
```rust
//! Percent discounts.

pub fn apply(total: u64, percent: u64) -> u64 {
    total - total * percent / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac1_ten_percent_off() {
        assert_eq!(apply(10_000, 10), 9_000);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn ac2_rejects_more_than_all() {
        apply(100, 200);
    }

    mod rounding {
        use super::*;

        #[test]
        fn ac3_rounds_down() {
            assert_eq!(apply(999, 10), 900);
        }
    }

    fn describe() -> &'static str {
        "#[test] fn not_a_test() {}"
    }
}
```

Create `crates/factory-presets/vectors/sources/checkout.rs` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/sources/checkout.rs -->
```rust
use cart::apply;

#[test]
fn ac4_checkout_totals() {
    assert_eq!(apply(20_000, 10), 18_000);
}
```

Create `crates/factory-presets/vectors/sources/generated.rs` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/sources/generated.rs -->
```rust
macro_rules! percent_cases {
    ($($name:ident: $percent:expr,)*) => {
        $(
            #[test]
            fn $name() {
                assert!(super::apply(100, $percent) <= 100);
            }
        )*
    };
}

#[cfg(test)]
mod tests {
    percent_cases! {
        ac1_ten: 10,
        ac1_twenty: 20,
    }
}
```

Create `crates/factory-presets/vectors/sources/cart.test.ts` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/sources/cart.test.ts -->
```typescript
import { describe, it, test, expect } from 'vitest'
import { total } from './cart'

describe('cart', () => {
  it('ac1 applies a percent code', () => {
    expect(total(10000, 'SAVE10')).toBe(9000)
  })

  describe("unknown codes", function () {
    it.skip(`ac2 leaves the total unchanged`, async () => {
      expect(total(10000, 'NOPE')).toBe(10000)
    })
  })

  test('totals are never negative (ac3)', () => {
    expect(total(0, 'SAVE10')).toBeGreaterThanOrEqual(0)
  })
})

test('top level & "quoted"', () => {
  expect(/it\('x'\)/.test("it('x')")).toBe(true)
})
```

Create `crates/factory-presets/vectors/sources/cart.test.js` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/sources/cart.test.js -->
```javascript
const { describe, it, test } = require('node:test')
const assert = require('node:assert/strict')
const { total } = require('../src/cart')

describe('cart', () => {
  it('ac1 applies a percent code', () => {
    assert.equal(total(10000, 'SAVE10'), 9000)
  })

  describe('unknown codes', () => {
    it('ac2 leaves the total unchanged', () => {
      assert.equal(total(10000, 'NOPE'), 10000)
    })
  })
})

test('top level', { todo: 'later' }, () => {})
```

Create `crates/factory-presets/vectors/sources/dynamic.test.js` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/sources/dynamic.test.js -->
```javascript
const { it } = require('node:test')
const assert = require('node:assert/strict')
const { total } = require('../src/cart')

it('ac1 applies a percent code', () => {
  assert.equal(total(10000, 'SAVE10'), 9000)
})

for (const percent of [10, 20]) {
  it(`ac2 takes ${percent} percent off`, () => {
    assert.ok(total(10000, `SAVE${percent}`) < 10000)
  })
}
```

Create `crates/factory-presets/vectors/sources/add.ts` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/sources/add.ts -->
```typescript
export function add(a: number, b: number): number {
  return a + b
}

if (import.meta.vitest) {
  const { it, expect } = import.meta.vitest
  it('ac1 adds', () => {
    expect(add(1, 2)).toBe(3)
  })
}
```

Create `crates/factory-presets/vectors/manifests/cargo-before.toml` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/manifests/cargo-before.toml -->
```toml
[package]
name = "cart"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
proptest = "1"
```

Create `crates/factory-presets/vectors/manifests/cargo-added.toml` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/manifests/cargo-added.toml -->
```toml
[package]
name = "cart"
version = "0.1.0"
edition = "2021"

[dependencies]
rust_decimal = "1"
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
insta = { version = "1" }
proptest = "1"
```

Create `crates/factory-presets/vectors/manifests/cargo-test-target.toml` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/manifests/cargo-test-target.toml -->
```toml
[package]
name = "cart"
version = "0.1.0"
edition = "2021"

[dependencies]
rust_decimal = "1"
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
proptest = "1"

[[test]]
name = "only"
path = "tests/only.rs"
```

Create `crates/factory-presets/vectors/manifests/cargo-git.toml` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/manifests/cargo-git.toml -->
```toml
[package]
name = "cart"
version = "0.1.0"
edition = "2021"

[dependencies]
rust_decimal = { git = "https://example.com/rust_decimal.git" }
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
proptest = "1"
```

Create `crates/factory-presets/vectors/manifests/package-before.json` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/manifests/package-before.json -->
```json
{
  "name": "cart",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "test": "node --test"
  },
  "dependencies": {
    "decimal.js": "^10.4.3"
  },
  "devDependencies": {
    "vitest": "^3.0.0"
  }
}
```

Create `crates/factory-presets/vectors/manifests/package-added.json` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/manifests/package-added.json -->
```json
{
  "name": "cart",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "test": "node --test"
  },
  "dependencies": {
    "decimal.js": "^10.4.3",
    "zod": "^3.23.0"
  },
  "devDependencies": {
    "@types/node": "^22.0.0",
    "vitest": "^3.0.0"
  }
}
```

Create `crates/factory-presets/vectors/manifests/package-script.json` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/manifests/package-script.json -->
```json
{
  "name": "cart",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "postinstall": "node scripts/setup.js",
    "test": "node --test"
  },
  "dependencies": {
    "decimal.js": "^10.4.3",
    "zod": "^3.23.0"
  },
  "devDependencies": {
    "vitest": "^3.0.0"
  }
}
```

Create `crates/factory-presets/vectors/manifests/package-alias.json` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/manifests/package-alias.json -->
```json
{
  "name": "cart",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "test": "node --test"
  },
  "dependencies": {
    "decimal.js": "^10.4.3",
    "zod": "npm:not-zod@1.0.0"
  },
  "devDependencies": {
    "vitest": "^3.0.0"
  }
}
```

Create `crates/factory-presets/vectors/index.json` with exactly this content:

<!-- plan-op: create crates/factory-presets/vectors/index.json -->
```json
{
  "pairs": [
    {
      "name": "nextest",
      "preset": "cargo",
      "report": "reports/nextest.xml",
      "sources": [
        {"path": "crates/cart/src/discount.rs", "file": "sources/discount.rs"},
        {"path": "crates/cart/tests/checkout.rs", "file": "sources/checkout.rs"}
      ],
      "cases": [
        {"id": "cart > discount::tests::ac1_ten_percent_off", "status": "passed"},
        {"id": "cart > discount::tests::ac2_rejects_more_than_all", "status": "failed"},
        {"id": "cart > discount::tests::rounding::ac3_rounds_down", "status": "passed"},
        {"id": "cart::checkout > ac4_checkout_totals", "status": "passed"}
      ]
    },
    {
      "name": "vitest",
      "preset": "node",
      "report": "reports/vitest.xml",
      "sources": [
        {"path": "src/cart.test.ts", "file": "sources/cart.test.ts"}
      ],
      "cases": [
        {"id": "src/cart.test.ts > cart > ac1 applies a percent code", "status": "passed"},
        {"id": "src/cart.test.ts > cart > unknown codes > ac2 leaves the total unchanged", "status": "skipped"},
        {"id": "src/cart.test.ts > cart > totals are never negative (ac3)", "status": "failed"},
        {"id": "src/cart.test.ts > top level & \"quoted\"", "status": "passed"}
      ]
    },
    {
      "name": "node-test",
      "preset": "node",
      "report": "reports/node-test.xml",
      "sources": [
        {"path": "test/cart.test.js", "file": "sources/cart.test.js"}
      ],
      "cases": [
        {"id": "test/cart.test.js > cart > ac1 applies a percent code", "status": "passed"},
        {"id": "test/cart.test.js > cart > unknown codes > ac2 leaves the total unchanged", "status": "failed"},
        {"id": "test/cart.test.js > top level", "status": "skipped"}
      ]
    }
  ],
  "report_errors": [
    {"preset": "cargo", "report": "reports/duplicate.xml", "error": "duplicate_id"},
    {"preset": "node", "report": "sources/cart.test.js", "error": "malformed"}
  ],
  "enumerate_errors": [
    {"preset": "node", "path": "test/dynamic.test.js", "file": "sources/dynamic.test.js", "error": "not_literal"},
    {"preset": "cargo", "path": "crates/cart/src/generated.rs", "file": "sources/generated.rs", "error": "not_literal"},
    {"preset": "node", "path": "src/App.test.tsx", "file": "sources/cart.test.ts", "error": "unsupported_file"},
    {"preset": "cargo", "path": "src/discount.rs", "file": "sources/discount.rs", "error": "unknown_package"}
  ],
  "regions": [
    {"preset": "cargo", "path": "crates/cart/src/discount.rs", "file": "sources/discount.rs", "regions": [[109, 604]]},
    {"preset": "cargo", "path": "crates/cart/tests/checkout.rs", "file": "sources/checkout.rs", "regions": [[18, 97]]},
    {"preset": "node", "path": "src/add.ts", "file": "sources/add.ts", "regions": [[70, 201]]},
    {"preset": "node", "path": "src/cart.test.ts", "file": "sources/cart.test.ts", "regions": []}
  ],
  "criteria": [
    {"id": "cart > discount::tests::ac1_ten_percent_off", "criterion": "AC-1"},
    {"id": "cart > discount::tests::ac10_ten_percent_off", "criterion": "AC-10"},
    {"id": "cart > discount::tests::mac1_ten_percent_off", "criterion": null},
    {"id": "cart > discount::tests::ac1x", "criterion": null},
    {"id": "src/cart.test.ts > cart > totals are never negative (ac3)", "criterion": "AC-3"},
    {"id": "src/cart.test.ts > cart > AC-3 totals", "criterion": null},
    {"id": "test/ac7/cart.test.js > totals", "criterion": null}
  ],
  "protected": [
    {"preset": "cargo", "path": "build.rs", "protected": true},
    {"preset": "cargo", "path": "crates\\cart\\Build.rs", "protected": true},
    {"preset": "cargo", "path": ".cargo/config.toml", "protected": true},
    {"preset": "cargo", "path": "rust-toolchain.toml", "protected": true},
    {"preset": "cargo", "path": "rust-toolchain", "protected": true},
    {"preset": "cargo", "path": "crates/cart/src/lib.rs", "protected": false},
    {"preset": "cargo", "path": "crates/cart/Cargo.toml", "protected": false},
    {"preset": "cargo", "path": "vitest.config.ts", "protected": false},
    {"preset": "node", "path": "vitest.config.ts", "protected": true},
    {"preset": "node", "path": "packages/web/jest.config.js", "protected": true},
    {"preset": "node", "path": ".mocharc.json", "protected": true},
    {"preset": "node", "path": "packages/web/.npmrc", "protected": true},
    {"preset": "node", "path": "src/cart.js", "protected": false},
    {"preset": "node", "path": "package.json", "protected": false},
    {"preset": "node", "path": "build.rs", "protected": false},
    {"preset": "cargo", "path": ".claude/settings.json", "protected": true},
    {"preset": "node", "path": "packages/web/.claude/agents/x.md", "protected": true},
    {"preset": "cargo", "path": ".mcp.json", "protected": true},
    {"preset": "node", "path": "docs/AGENTS.md", "protected": true},
    {"preset": "cargo", "path": "crates/cart/claude.md", "protected": true},
    {"preset": "node", "path": "src/claude/index.js", "protected": false}
  ],
  "manifests": [
    {"preset": "cargo", "path": "crates/cart/Cargo.toml", "before": "manifests/cargo-before.toml", "after": "manifests/cargo-before.toml", "added": []},
    {"preset": "cargo", "path": "crates/cart/Cargo.toml", "before": "manifests/cargo-before.toml", "after": "manifests/cargo-added.toml", "added": ["insta", "rust_decimal"]},
    {"preset": "cargo", "path": "crates/cart/Cargo.toml", "before": "manifests/cargo-before.toml", "after": "manifests/cargo-test-target.toml", "error": "other_change"},
    {"preset": "cargo", "path": "crates/cart/Cargo.toml", "before": "manifests/cargo-before.toml", "after": "manifests/cargo-git.toml", "error": "unsupported_source"},
    {"preset": "cargo", "path": "crates/cart/Cargo.toml", "before": "manifests/cargo-added.toml", "after": "manifests/cargo-before.toml", "error": "dependency_removed"},
    {"preset": "node", "path": "package.json", "before": "manifests/package-before.json", "after": "manifests/package-added.json", "added": ["@types/node", "zod"]},
    {"preset": "node", "path": "package.json", "before": "manifests/package-before.json", "after": "manifests/package-script.json", "error": "other_change"},
    {"preset": "node", "path": "package.json", "before": "manifests/package-before.json", "after": "manifests/package-alias.json", "error": "unsupported_source"},
    {"preset": "node", "path": "package.json", "before": "manifests/cargo-before.toml", "after": "manifests/package-added.json", "error": "malformed"},
    {"preset": "cargo", "path": "package.json", "before": "manifests/package-before.json", "after": "manifests/package-added.json", "error": "not_a_manifest"}
  ],
  "secrets": [
    {"rule": "aws-access-key-id", "parts": ["AK", "IA", "ABCDEFGHIJKLMNOP"], "matches": true},
    {"rule": "aws-access-key-id", "parts": ["AK", "IA", "ABCDEFGH"], "matches": false},
    {"rule": "github-token", "parts": ["gh", "p_", "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ"], "matches": true},
    {"rule": "github-token", "parts": ["gh", "x_", "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ"], "matches": false},
    {"rule": "github-fine-grained-token", "parts": ["github", "_pat_", "abcdefghijklmnopqrstuv", "_", "abcdefghijklmnopqrst"], "matches": true},
    {"rule": "anthropic-api-key", "parts": ["sk-", "ant-", "api03-", "abcdefghijklmnopqrstuvwxyzABCD"], "matches": true},
    {"rule": "openai-api-key", "parts": ["sk-", "proj-", "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN"], "matches": true},
    {"rule": "openai-api-key", "parts": ["sk-", "short"], "matches": false},
    {"rule": "slack-token", "parts": ["xo", "xb-", "123456789012-", "abcdefghijkl"], "matches": true},
    {"rule": "stripe-live-key", "parts": ["sk_", "live_", "abcdefghijklmnopqrstuvwx"], "matches": true},
    {"rule": "stripe-live-key", "parts": ["sk_", "test_", "abcdefghijklmnopqrstuvwx"], "matches": false},
    {"rule": "google-api-key", "parts": ["AI", "za", "abcdefghijklmnopqrstuvwxyzABCDEFGHI"], "matches": true},
    {"rule": "npm-token", "parts": ["np", "m_", "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ"], "matches": true},
    {"rule": "private-key-block", "parts": ["-----BEGIN RSA ", "PRIVATE KEY-----"], "matches": true},
    {"rule": "private-key-block", "parts": ["-----BEGIN ", "PUBLIC KEY-----"], "matches": false}
  ]
}
```

- [ ] **Step 2: Write the failing tests for the vector runner**

In `crates/factory-presets/src/testkit.rs`, add this line and the blank line after it, directly above the `use crate::types::...;` line:

<!-- plan-op: before-line crates/factory-presets/src/testkit.rs :: use crate::types::{test_id, TestCase, TestStatus, ID_SEPARATOR}; -->
```rust
pub mod vectors;

```

Create `crates/factory-presets/src/testkit/vectors.rs` holding only its test module:

<!-- plan-op: create crates/factory-presets/src/testkit/vectors.rs -->
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shared_vector_passes() {
        // 3 pairs, 2 report errors, 4 enumerate errors, 4 region cases, 7 criterion cases,
        // 21 protected-path cases, 10 manifest cases. Dropping a vector changes this number.
        assert_eq!(run_all(), Ok(51));
    }

    #[test]
    fn every_secret_vector_passes_with_the_regex_crate() {
        let is_match =
            |pattern: &str, text: &str| regex::Regex::new(pattern).unwrap().is_match(text);
        assert_eq!(run_secrets(is_match), Ok(15));
    }

    #[test]
    fn every_embedded_file_is_named_by_the_index_and_none_is_empty() {
        assert!(!INDEX.is_empty());
        for (name, text) in FILES {
            assert!(!text.is_empty(), "{name} is empty");
            assert!(
                INDEX.contains(name),
                "{name} is embedded and not in the index"
            );
        }
    }

    #[test]
    fn no_vector_file_matches_a_secret_rule() {
        for (name, text) in FILES.iter().chain(&[("index.json", INDEX)]) {
            for rule in secret_rules() {
                let found = regex::Regex::new(rule.pattern).unwrap().is_match(text);
                assert!(!found, "{name} matches secret rule {}", rule.id);
            }
        }
    }
}
```

- [ ] **Step 3: Run the tests and watch them fail**

Run: `cargo test -p factory-presets`

Expected: FAIL. The crate does not compile; the first error is

```text
error[E0425]: cannot find value `INDEX` in this scope
```

and more errors of the same kind follow, one for each name the tests use that does not exist yet.

- [ ] **Step 4: Write the implementation of the vector runner**

Add this to `crates/factory-presets/src/testkit/vectors.rs`, directly above its `#[cfg(test)]` line:

<!-- plan-op: above-tests crates/factory-presets/src/testkit/vectors.rs -->
```rust
//! The shared test vectors: data files under `vectors/`, compiled into the crate, and the
//! functions that run them.
//!
//! Two repositories link this crate. Each runs [`run_all`] and [`run_secrets`] in a test, so
//! both prove the same behaviour on the same data. The files are embedded at compile time;
//! nothing is read from disk when the vectors run.
//!
//! The reports and sources here are ASSUMED shapes until spike S6 replaces them with
//! captured ones (see the crate's plan, "Reconcile with spike S6").

use crate::types::{TestCase, TestStatus};
use crate::{preset, secret_rules, Preset};
use serde::Deserialize;
use std::collections::BTreeSet;

/// `vectors/index.json`: which cases exist and what each expects.
pub const INDEX: &str = include_str!("../../vectors/index.json");

/// Every data file under `vectors/`, by its path relative to that directory.
pub const FILES: &[(&str, &str)] = &[
    (
        "reports/nextest.xml",
        include_str!("../../vectors/reports/nextest.xml"),
    ),
    (
        "reports/vitest.xml",
        include_str!("../../vectors/reports/vitest.xml"),
    ),
    (
        "reports/node-test.xml",
        include_str!("../../vectors/reports/node-test.xml"),
    ),
    (
        "reports/duplicate.xml",
        include_str!("../../vectors/reports/duplicate.xml"),
    ),
    (
        "sources/discount.rs",
        include_str!("../../vectors/sources/discount.rs"),
    ),
    (
        "sources/checkout.rs",
        include_str!("../../vectors/sources/checkout.rs"),
    ),
    (
        "sources/generated.rs",
        include_str!("../../vectors/sources/generated.rs"),
    ),
    (
        "sources/cart.test.ts",
        include_str!("../../vectors/sources/cart.test.ts"),
    ),
    (
        "sources/cart.test.js",
        include_str!("../../vectors/sources/cart.test.js"),
    ),
    (
        "sources/dynamic.test.js",
        include_str!("../../vectors/sources/dynamic.test.js"),
    ),
    (
        "sources/add.ts",
        include_str!("../../vectors/sources/add.ts"),
    ),
    (
        "manifests/cargo-before.toml",
        include_str!("../../vectors/manifests/cargo-before.toml"),
    ),
    (
        "manifests/cargo-added.toml",
        include_str!("../../vectors/manifests/cargo-added.toml"),
    ),
    (
        "manifests/cargo-test-target.toml",
        include_str!("../../vectors/manifests/cargo-test-target.toml"),
    ),
    (
        "manifests/cargo-git.toml",
        include_str!("../../vectors/manifests/cargo-git.toml"),
    ),
    (
        "manifests/package-before.json",
        include_str!("../../vectors/manifests/package-before.json"),
    ),
    (
        "manifests/package-added.json",
        include_str!("../../vectors/manifests/package-added.json"),
    ),
    (
        "manifests/package-script.json",
        include_str!("../../vectors/manifests/package-script.json"),
    ),
    (
        "manifests/package-alias.json",
        include_str!("../../vectors/manifests/package-alias.json"),
    ),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    pairs: Vec<Pair>,
    report_errors: Vec<ReportErrorCase>,
    enumerate_errors: Vec<EnumerateErrorCase>,
    regions: Vec<RegionCase>,
    criteria: Vec<CriterionCase>,
    protected: Vec<ProtectedCase>,
    manifests: Vec<ManifestCase>,
    secrets: Vec<SecretCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceRef {
    /// The repository-relative path the source is enumerated as.
    path: String,
    /// The vector file that holds it.
    file: String,
}

/// A report and the sources that produced it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    name: String,
    preset: String,
    report: String,
    sources: Vec<SourceRef>,
    /// What `read_report` returns, in order.
    cases: Vec<ExpectedCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCase {
    id: String,
    status: TestStatus,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportErrorCase {
    preset: String,
    report: String,
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumerateErrorCase {
    preset: String,
    path: String,
    file: String,
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegionCase {
    preset: String,
    path: String,
    file: String,
    /// `[start, end]` byte offsets, end exclusive.
    regions: Vec<(usize, usize)>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CriterionCase {
    id: String,
    criterion: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedCase {
    preset: String,
    path: String,
    protected: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestCase {
    preset: String,
    path: String,
    before: String,
    after: String,
    /// Expected added names; absent when `error` is expected.
    #[serde(default)]
    added: Option<Vec<String>>,
    #[serde(default)]
    error: Option<String>,
}

/// One example for one secret rule. The example is stored in parts and joined when the
/// vector runs, so no file in the repository holds a whole token-shaped string.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretCase {
    rule: String,
    parts: Vec<String>,
    matches: bool,
}

fn read_index() -> Result<Index, Vec<String>> {
    serde_json::from_str(INDEX).map_err(|e| vec![format!("vectors/index.json does not read: {e}")])
}

fn file(name: &str, failures: &mut Vec<String>) -> Option<&'static str> {
    let found = FILES
        .iter()
        .find(|(file, _)| *file == name)
        .map(|(_, text)| *text);
    match found {
        Some(text) if text.contains('\r') => {
            failures.push(format!(
                "{name}: has CR line endings; vectors are LF (see vectors/.gitattributes)"
            ));
            None
        }
        Some(text) => Some(text),
        None => {
            failures.push(format!("{name}: named by the index and not embedded"));
            None
        }
    }
}

fn find_preset(name: &str, failures: &mut Vec<String>) -> Option<&'static dyn Preset> {
    let found = preset(name);
    if found.is_none() {
        failures.push(format!("the index names an unknown preset {name:?}"));
    }
    found
}

fn check_pair(pair: &Pair, failures: &mut Vec<String>) {
    let name = &pair.name;
    let (Some(p), Some(report)) = (
        find_preset(&pair.preset, failures),
        file(&pair.report, failures),
    ) else {
        return;
    };
    let expected: Vec<TestCase> = pair
        .cases
        .iter()
        .map(|c| TestCase {
            id: c.id.clone(),
            status: c.status,
        })
        .collect();
    match p.read_report(report) {
        Ok(cases) if cases == expected => {}
        Ok(cases) => failures.push(format!("{name}: read_report gave {cases:?}")),
        Err(error) => failures.push(format!("{name}: read_report failed: {error}")),
    }
    let mut enumerated: BTreeSet<String> = BTreeSet::new();
    for source in &pair.sources {
        let Some(text) = file(&source.file, failures) else {
            return;
        };
        match p.enumerate_ids(&source.path, text) {
            Ok(ids) => enumerated.extend(ids),
            Err(error) => failures.push(format!("{name}: {}: {error}", source.path)),
        }
    }
    let reported: BTreeSet<String> = pair.cases.iter().map(|c| c.id.clone()).collect();
    if enumerated != reported {
        failures.push(format!(
            "{name}: enumerated ids differ from reported ids; only enumerated: {:?}; only reported: {:?}",
            enumerated.difference(&reported).collect::<Vec<_>>(),
            reported.difference(&enumerated).collect::<Vec<_>>()
        ));
    }
}

fn check_manifest(case: &ManifestCase, failures: &mut Vec<String>) {
    let (Some(p), Some(before), Some(after)) = (
        find_preset(&case.preset, failures),
        file(&case.before, failures),
        file(&case.after, failures),
    ) else {
        return;
    };
    let label = format!("manifest {} -> {}", case.before, case.after);
    match (
        p.added_dependencies(&case.path, before, after),
        &case.added,
        &case.error,
    ) {
        (Ok(added), Some(expected), None) if &added == expected => {}
        (Err(error), None, Some(expected)) if error.code() == expected => {}
        (got, _, _) => failures.push(format!("{label}: got {got:?}")),
    }
}

/// Run every vector except the secret rules. `Ok` carries the number of cases checked; `Err`
/// carries one line per failed expectation.
pub fn run_all() -> Result<usize, Vec<String>> {
    let index = read_index()?;
    let mut failures = Vec::new();

    for pair in &index.pairs {
        check_pair(pair, &mut failures);
    }
    for case in &index.report_errors {
        let (Some(p), Some(report)) = (
            find_preset(&case.preset, &mut failures),
            file(&case.report, &mut failures),
        ) else {
            continue;
        };
        match p.read_report(report) {
            Err(error) if error.code() == case.error => {}
            got => failures.push(format!("report {}: got {got:?}", case.report)),
        }
    }
    for case in &index.enumerate_errors {
        let (Some(p), Some(text)) = (
            find_preset(&case.preset, &mut failures),
            file(&case.file, &mut failures),
        ) else {
            continue;
        };
        match p.enumerate_ids(&case.path, text) {
            Err(error) if error.code() == case.error => {}
            got => failures.push(format!("enumerate {}: got {got:?}", case.path)),
        }
    }
    for case in &index.regions {
        let (Some(p), Some(text)) = (
            find_preset(&case.preset, &mut failures),
            file(&case.file, &mut failures),
        ) else {
            continue;
        };
        let got: Vec<(usize, usize)> = p
            .test_regions(&case.path, text)
            .into_iter()
            .map(|r| (r.start, r.end))
            .collect();
        if got != case.regions {
            failures.push(format!("regions {}: got {got:?}", case.path));
        }
    }
    for case in &index.criteria {
        for name in ["cargo", "node"] {
            let got = preset(name).and_then(|p| p.criterion_of(&case.id));
            if got != case.criterion {
                failures.push(format!(
                    "criterion_of({:?}) on {name}: got {got:?}",
                    case.id
                ));
            }
        }
    }
    for case in &index.protected {
        let Some(p) = find_preset(&case.preset, &mut failures) else {
            continue;
        };
        if p.is_protected(&case.path) != case.protected {
            failures.push(format!(
                "is_protected({:?}) on {}: expected {}",
                case.path, case.preset, case.protected
            ));
        }
    }
    for case in &index.manifests {
        check_manifest(case, &mut failures);
    }

    if failures.is_empty() {
        Ok(index.pairs.len()
            + index.report_errors.len()
            + index.enumerate_errors.len()
            + index.regions.len()
            + index.criteria.len()
            + index.protected.len()
            + index.manifests.len())
    } else {
        Err(failures)
    }
}

/// Run the secret-rule vectors with the caller's pattern engine: `is_match(pattern, text)`
/// says whether the pattern finds a match in the text. This crate links no such engine.
pub fn run_secrets(is_match: impl Fn(&str, &str) -> bool) -> Result<usize, Vec<String>> {
    let index = read_index()?;
    let mut failures = Vec::new();
    for case in &index.secrets {
        let Some(rule) = secret_rules().iter().find(|r| r.id == case.rule) else {
            failures.push(format!("the index names an unknown rule {:?}", case.rule));
            continue;
        };
        let text = format!("value = \"{}\"", case.parts.concat());
        if is_match(rule.pattern, &text) != case.matches {
            failures.push(format!(
                "rule {} on parts {:?}: expected matches = {}",
                case.rule, case.parts, case.matches
            ));
        }
    }
    for rule in secret_rules() {
        if !index.secrets.iter().any(|c| c.rule == rule.id && c.matches) {
            failures.push(format!("rule {} has no matching example", rule.id));
        }
    }
    if failures.is_empty() {
        Ok(index.secrets.len())
    } else {
        Err(failures)
    }
}
```

If a vector test fails, its message lists each failed expectation. Fix the code or the data that is wrong; never change `51` or `15` to make it pass.

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p factory-presets`

Expected: PASS, ending with `test result: ok. 127 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add crates/factory-presets/vectors crates/factory-presets/src/testkit.rs crates/factory-presets/src/testkit/vectors.rs
git commit -m "test(factory-presets): shared vectors and the runners a second repository can call"
```


### Task 24: Verify the lane

**Files:**
- No file changes.

**Interfaces:**
- Consumes: Tasks 11 to 23.
- Produces: a lane that passes every gate on the assumed fixtures, ready to be reconciled with spike S6.

- [ ] **Step 1: Run the tests, with and without the feature**

```bash
cargo test -p factory-presets
cargo test -p factory-presets --features testkit
```

Expected, both times: `test result: ok. 127 passed; 0 failed; 0 ignored`.

- [ ] **Step 2: Run the format and lint gates**

```bash
cargo fmt --all -- --check
cargo clippy -p factory-presets --all-targets -- -D warnings
cargo clippy -p factory-presets --all-targets --features testkit -- -D warnings
```

Expected: `cargo fmt` prints nothing and exits 0; each `cargo clippy` ends with a `Finished` line and no warning. If `cargo fmt` reports a difference in a file of this lane, run `cargo fmt -p factory-presets`, re-run the tests, and commit the result as `style(factory-presets): cargo fmt`.

- [ ] **Step 3: Check purity and ownership**

```bash
grep -rnE "std::(fs|net|process|env|thread)|^use tokio|\.await" crates/factory-presets/src; echo "grep exit: $?"
git diff --stat origin/factory/m0 -- . ":(exclude)crates/factory-presets"
git status --short
```

Expected: the `grep` prints only `grep exit: 1` (no match; the `#[tokio::test]` inside a test fixture string is not matched by this pattern); the `git diff --stat` prints nothing (no file outside the lane changed, `Cargo.lock` included); `git status --short` prints nothing.


### Task 25: Reconcile with spike S6

**Files:**
- Replace: `crates/factory-presets/vectors/reports/nextest.xml`, `crates/factory-presets/vectors/reports/vitest.xml`, `crates/factory-presets/vectors/reports/node-test.xml`
- Replace, only if the spike ran different sources: `crates/factory-presets/vectors/sources/discount.rs`, `crates/factory-presets/vectors/sources/checkout.rs`, `crates/factory-presets/vectors/sources/cart.test.ts`, `crates/factory-presets/vectors/sources/cart.test.js`
- Modify: `crates/factory-presets/vectors/index.json` (the `pairs` and `regions` entries for the replaced files)
- Modify, only if a shape differs: `crates/factory-presets/src/presets.rs` (`cargo_id`, `node_id` and the three report constants in its tests), `crates/factory-presets/src/cargo.rs` (`locate` and its two path tests), `crates/factory-presets/src/testkit.rs` (`ReportBuilder::build`)

**Interfaces:**
- Consumes: spike S6's captured JUnit reports and the sources that produced them (in the spike's scratch directory, `D:\MajorProjects\.swarm-wt\spike-s6`), and its findings; spike S7's findings, for one question.
- Produces: the same public interface as before, with fixtures that are captures instead of assumptions, and the pull request.

This task turns assumption into fact, or stops. Three rules govern it:

1. **A captured report is evidence. Never edit one to make a test pass.** The only edits allowed to a captured file are scrubbing: a host name, a timestamp, or a machine-specific absolute path prefix, each replaced by a fixed placeholder, and each noted in the commit message.
2. **Expected values come from the report, read by a person, not from running the code under test.** Do not fill `index.json` by printing what `read_report` returns.
3. **If the spike shows that ids cannot be matched for a runner, stop.** That is the case S6 exists to find, and the remedy (a pinned tool version, a reporter option, a wider signature) is the owner's decision.

- [ ] **Step 1: Read what the spike found, against assumptions A1 to A8**

```bash
ls /d/MajorProjects/.swarm-wt/spike-s6
grep -rn "ASSUM" crates/factory-presets/src crates/factory-presets/vectors | wc -l
grep -rn "ASSUM" crates/factory-presets/src | cut -d: -f1 | sort | uniq -c
```

Expected: the spike's directory lists its captured reports and the sources it ran; the second command prints a non-zero count of `ASSUMPTION`/`ASSUMED` markers; the third names the files that carry them (`cargo.rs`, `node.rs`, `presets.rs`, `testkit.rs`, `testkit/vectors.rs`).

Write down, for each of A1 to A8 in the table at the top of this lane, one of: confirmed, contradicted (and how), or not covered by the spike. If A3 or A7 is contradicted, or anything is not covered, stop here and report: the rest of this task cannot be done honestly.

- [ ] **Step 2: Replace the three hand-written reports with the captured ones**

Copy the spike's captured reports over the assumed ones, then scrub only host names, timestamps and absolute path prefixes:

```bash
S6=/d/MajorProjects/.swarm-wt/spike-s6
cp "$S6/nextest/junit.xml"   crates/factory-presets/vectors/reports/nextest.xml
cp "$S6/vitest/junit.xml"    crates/factory-presets/vectors/reports/vitest.xml
cp "$S6/node-test/junit.xml" crates/factory-presets/vectors/reports/node-test.xml
grep -c $'\r' crates/factory-presets/vectors/reports/*.xml
grep -nE 'hostname="|timestamp="|[A-Za-z]:[\\/]|/home/|/Users/' crates/factory-presets/vectors/reports/*.xml | head -20
```

Expected: the three `cp` commands succeed (if the spike laid its files out differently, use its paths; the three destinations do not change). The `grep -c` prints `0` for every file; if a file has CR line endings, convert it to LF (`sed -i 's/\r$//' <file>`) because the vector runner refuses CR. The last `grep` shows what there is to scrub: replace each host name with `check`, each timestamp with `2026-10-04T10:00:00.000Z`, and remove each machine-specific path prefix so the path is repository-relative. Leave `reports/duplicate.xml` as it is: it is a made-up error case, not a capture.

- [ ] **Step 3: Make the sources match what the spike ran**

If the spike ran this lane's four test sources unchanged, there is nothing to do. Check:

```bash
S6=/d/MajorProjects/.swarm-wt/spike-s6
diff "$S6/nextest/crates/cart/src/discount.rs"   crates/factory-presets/vectors/sources/discount.rs
diff "$S6/nextest/crates/cart/tests/checkout.rs" crates/factory-presets/vectors/sources/checkout.rs
diff "$S6/vitest/src/cart.test.ts"               crates/factory-presets/vectors/sources/cart.test.ts
diff "$S6/node-test/test/cart.test.js"           crates/factory-presets/vectors/sources/cart.test.js
```

Expected: no output from any `diff`. Where there is output, the spike's file is the one that produced the captured report, so it wins: copy it over the vector file, and note the repository-relative path it had in the spike (it goes into `index.json` in the next step).

- [ ] **Step 4: Update the index from the reports, by reading them**

Open each replaced report and `crates/factory-presets/vectors/index.json` side by side. For each of the three entries under `"pairs"`, rewrite `"cases"` so that it lists, in document order, one object per `<testcase>` in the report. An entry has this form and no other fields:

```json
{"id": "<class> > <name>", "status": "passed"}
```

`status` is `passed`, `failed`, `errored` or `skipped`, read off the test case's child elements. `id` is the canonical id **as the decided rule defines it**: the class name and the test name the report gives, joined by `" > "`. Under each pair's `"sources"`, set every `"path"` to the repository-relative path that source had in the spike's tree.

If a replaced source file changed, also recompute its entry under `"regions"`: the byte offset of the first character of each test region and one past its last, for example with:

```bash
grep -b -o "#\[cfg(test)\]" crates/factory-presets/vectors/sources/discount.rs
wc -c crates/factory-presets/vectors/sources/discount.rs
```

Expected: the first command prints `<offset>:#[cfg(test)]`, the region's start; for a file that ends with the module's closing brace and one newline, the region's end is the byte count minus 1. Then update the comment in `crates/factory-presets/src/testkit/vectors.rs`'s `every_shared_vector_passes` if the number of cases changed, and the number itself, counting by hand from the index.

- [ ] **Step 5: Run the tests and sort the failures**

```bash
cargo test -p factory-presets 2>&1 | grep -E "^test .* FAILED|test result"
```

Expected: either everything passes (the assumed shapes were right), or failures. Sort each failing test into one of two groups.

**Group 1: tests that encode an assumed shape.** These may change, together with the code they pin, to match the captures:

| Test | Assumption it encodes | Code that changes with it |
|---|---|---|
| `presets::tests::cargo_ids_are_classname_then_name` | A1 | `presets::cargo_id`, the `NEXTEST` constant |
| `presets::tests::vitest_ids_are_the_file_then_the_reported_name` | A5 | `presets::node_id`, the `VITEST` constant |
| `presets::tests::node_test_ids_are_the_file_then_the_suites_then_the_name` | A6, A7 | `presets::node_id`, the `NODE_TEST` constant |
| `presets::tests::enumerated_ids_equal_reported_ids_on_the_assumed_fixtures` | A1 to A7 | whichever of the above changed |
| `cargo::tests::the_class_and_module_path_follow_the_files_place_in_the_package` | A2, A3, A4 | `cargo::locate` |
| `cargo::tests::a_file_with_no_package_directory_is_refused_only_if_it_declares_tests` | A3 | `cargo::locate` |
| `cargo::tests::enumerates_in_file_tests_with_their_module_path` | A2, A4 | `cargo::locate` |
| `testkit::tests::a_nextest_report_reads_back_through_the_cargo_preset`, `testkit::tests::vitest_and_node_test_reports_read_back_through_the_node_preset`, `testkit::tests::names_with_xml_special_characters_survive_the_round_trip` | A1, A5, A6, A7 | `testkit::ReportBuilder::build` |
| `testkit::vectors::tests::every_shared_vector_passes` | all | the three functions above; **never** the captured files |

**Group 2: every other test in the crate.** These must still pass, unchanged: all of `types`, `junit`, `marker`, `protected`, `rust_scan`, `js_scan`, `manifest` and `secrets`; in `cargo` and `node`, every test not listed above; in `presets`, `lookup_is_by_exact_name`, `a_preset_reference_can_cross_threads`, `an_id_reported_twice_is_an_error_not_two_results`, `a_node_test_report_without_file_attributes_cannot_tell_two_files_apart`, `the_same_test_name_in_two_classes_is_two_ids`, `a_case_without_the_attribute_its_id_needs_is_refused`, `report_errors_pass_through_both_presets` and `each_method_reaches_its_presets_own_rules`; and `testkit::vectors::tests::every_secret_vector_passes_with_the_regex_crate`, `every_embedded_file_is_named_by_the_index_and_none_is_empty` and `no_vector_file_matches_a_secret_rule`.

A failure in Group 2 is a defect this task introduced, or a finding the spike did not predict. Fix the defect; report the finding.

- [ ] **Step 6: For each Group 1 failure: change the test first, then the mapping**

Work one assumption at a time, test first. For example, if the spike shows nextest's `classname` for an integration test is `cart::tests/checkout` where this plan assumed `cart::checkout`, first change the expectation:

```rust
        assert_eq!(id("crates/cart/tests/checkout.rs"), "cart::tests/checkout > t");
```

run `cargo test -p factory-presets the_class_and_module_path` and watch that line fail, then change the one arm of `locate` in `crates/factory-presets/src/cargo.rs` that builds it:

```rust
        ("tests", [file]) => (format!("{package}::tests/{}", stem(file)), Vec::new()),
```

and run the test again and watch it pass. Do the same for the constant in `presets.rs` that shows the shape (replace the hand-written XML with a trimmed copy of the capture) and for `ReportBuilder::build`.

The two values in that example are illustrations of the *procedure*; the real ones come from the spike. Commit each assumption separately:

```bash
git add crates/factory-presets
git commit -m "fix(factory-presets): nextest integration-test class name, as captured by spike S6"
```

- [ ] **Step 7: Stop conditions**

Stop, commit nothing further, and report to the owner if any of these holds. Each needs a decision this lane cannot make.

```bash
grep -c 'file="' crates/factory-presets/vectors/reports/node-test.xml
grep -o 'file="[^"]*"' crates/factory-presets/vectors/reports/node-test.xml | sort -u | head -5
grep -o 'classname="[^"]*"' crates/factory-presets/vectors/reports/vitest.xml | sort -u | head -5
```

- **The captured `node --test` report has no `file` attribute** (the first command prints `0`). Then two files with the same test names cannot be told apart (assumption A7; the test `a_node_test_report_without_file_attributes_cannot_tell_two_files_apart` shows the consequence). Options for the owner: pin a Node release whose reporter writes it, or write the report another way.
- **The `file` values are absolute paths** (the second command shows a drive letter or a leading `/`). `read_report(&self, junit_xml: &str)` has no way to know the prefix to strip. Options: give the method the root, or fix the working directory the check container runs in.
- **Vitest's `classname` is not the repository-relative path** (the third command shows a path that starts below the repository root). Same shape of problem (open question Q6).
- **The package name is not the directory name** for the cargo sandbox (assumption A3), or the sandbox package sits at the repository root.
- **The spike reports that enumerated ids cannot equal reported ids** for some construct this plan enumerates.

- [ ] **Step 8: Mark what is now fact**

For each assumption the spike confirmed, change its marker in the source from an assumption to a dated statement of fact; leave the marker where the spike did not confirm it.

```bash
grep -rn "ASSUM" crates/factory-presets/src crates/factory-presets/vectors
```

For every line that command prints, either rewrite the sentence to begin `Confirmed by spike S6 (2026-10):` and state what was observed, or leave it and add the reason it is still an assumption. For example the header of `crates/factory-presets/src/cargo.rs` changes from

```rust
//! ASSUMPTION (spike S6 has not confirmed it): the test runner is cargo-nextest, and its
```

to

```rust
//! Confirmed by spike S6 (2026-10): the test runner is cargo-nextest, and its
```

Then rename the one test whose name says "assumed" and re-run:

```bash
sed -i 's/enumerated_ids_equal_reported_ids_on_the_assumed_fixtures/enumerated_ids_equal_reported_ids/' crates/factory-presets/src/presets.rs
cargo test -p factory-presets
git add crates/factory-presets
git commit -m "docs(factory-presets): mark the report shapes confirmed by spike S6"
```

Expected: every test passes. Read spike S7's findings for one question only: does either stack need a preset rule for running setup with lifecycle scripts off? This plan builds none (D15). If S7 says one is needed, report it as a request; do not add a trait method.

- [ ] **Step 9: Run every gate again**

```bash
cargo test -p factory-presets
cargo test -p factory-presets --features testkit
cargo fmt --all -- --check
cargo clippy -p factory-presets --all-targets -- -D warnings
cargo clippy -p factory-presets --all-targets --features testkit -- -D warnings
grep -rnE "std::(fs|net|process|env|thread)|^use tokio|\.await" crates/factory-presets/src; echo "grep exit: $?"
git diff --stat origin/factory/m0 -- . ":(exclude)crates/factory-presets"
git status --short
```

Expected: both test runs end `test result: ok.` with 0 failed; `cargo fmt` prints nothing; both `cargo clippy` runs end with `Finished` and no warning; the `grep` prints only `grep exit: 1`; the `git diff --stat` and `git status --short` print nothing.

- [ ] **Step 10: Push and open the pull request**

```bash
git push -u origin feat/m0-factory-presets
gh pr create --base factory/m0 --head feat/m0-factory-presets \
  --title "feat(factory-presets): the cargo and node presets" \
  --body "$(cat <<'EOF'
## What changed

New pure crate `crates/factory-presets` (lane CC-PRESETS of factory milestone 0):

- `Preset` trait with two implementations behind `preset("cargo")` and `preset("node")`.
- Reads a JUnit XML report into test ids and outcomes, as a bounded event stream.
- Enumerates test ids from Rust and from JavaScript/TypeScript source; refuses a file whose test names are not literals.
- Criterion markers, protected path patterns, byte ranges of in-file test code.
- Reads `Cargo.toml` and `package.json` before and after a change: added dependencies, and an error for any other difference.
- A fixed list of ten secret-detection rules, as data.
- A `testkit` feature with `ReportBuilder`, and shared vectors under `vectors/`.

No file outside `crates/factory-presets/` is touched.

## Report shapes

The JUnit fixtures under `vectors/reports/` are the reports captured by spike S6. Which assumptions the spike confirmed, and which it did not cover, are listed in this lane's final report.

## How it was verified

- `cargo test -p factory-presets` and `cargo test -p factory-presets --features testkit`
- `cargo fmt --all -- --check`
- `cargo clippy -p factory-presets --all-targets -- -D warnings`, with and without `--features testkit`
- a grep of `src/` for file, network, process, environment, thread and async use: no match

## Deviations from the shared-interface draft, for the reviewer

D8 to D15 in the plan `docs/superpowers/plans/2026-10-04-factory-m0-spec-and-presets.md`.

## Issue

None: this is lane CC-PRESETS of the factory program's milestone 0.
EOF
)"
```

Expected: `gh` prints the pull request's URL. A repository hook inspects `gh pr create`; if it blocks the command, say so in your report and leave it blocked. Leave the pull request open for the reviewer.

- [ ] **Step 11: Write the final report**

End with a report: the files you added or replaced, the pull request link, what each Verify command actually printed (including anything that failed), and, for each of A1 to A8, whether spike S6 confirmed it, contradicted it or did not cover it, with what you changed as a result. Add each scrub you applied to a captured report, anything you need from the coordinator, the judgement calls you made and why, and anything other lanes should know. Repeat deviations D8 to D15 and open questions Q4 to Q8.

```bash
git log --oneline origin/factory/m0..HEAD
```

Expected: one commit per red-green cycle of Tasks 11 to 23 (sixteen), then this task's commits.


## Self-review

**What was run.** The code in this plan was not written into the plan and hoped over. It was built first as two working crates in a scratch workspace outside every repository, against the dependency versions requested above (`serde-saphyr` 1.3.0, `sha2` 0.10.9, `quick-xml` 0.42.0, `toml` 1.1.6, `serde_json` 1.0.151, `regex` 1.13.1, on rustc 1.93.1, Windows 11). The plan's steps were then cut from that code by a script, and checked as follows on 2026-10-04.

| # | Check | Result |
|---|---|---|
| 1 | **Replay.** Every step of Tasks 1 to 9 and 11 to 23 was applied, in order, to a fresh workspace holding only the two manifests and two one-line `lib.rs` files. After each "write the failing tests" step, `cargo test -p <crate>` was run and had to fail; after each "write the implementation" step it had to pass. The first error line and the pass count printed under each run step are the ones recorded | 25 red-green cycles; every one failed, then passed |
| 2 | **`cargo fmt --all -- --check` after every cycle** of the replay, so each intermediate `lib.rs` shown is already formatted | Clean every time |
| 3 | **Round trip through the document.** The finished Markdown was parsed back: every code block that follows a `plan-op` comment was applied to a second fresh workspace, and the resulting tree was compared byte for byte with the scratch crates | 103 blocks applied; 55 files identical |
| 4 | **The Verify commands, on the tree rebuilt from the document:** `cargo test -p <crate>` with and without `--features testkit`, `cargo clippy -p <crate> --all-targets -- -D warnings` with and without the feature, `cargo fmt --all -- --check` | `factory-spec`: 94 tests pass. `factory-presets`: 127 tests pass. No clippy warning, no format difference |
| 5 | **Purity grep** from Tasks 10 and 24 on that tree | No match in either crate |
| 6 | **No token-shaped string in this document.** Each of the ten secret patterns was run over this file | No match |
| 7 | **No placeholder.** This file was searched for "TBD", "TODO", "similar to Task", "add validation" and "fill in" | None outside this row |
| 8 | **Every test named in Review Focus exists** in the code under the task it is attributed to | All found |
| 9 | **Types.** Every type a task consumes is produced by an earlier task of the same lane; the two lanes share no type. Checked by the replay: each cycle compiles with only the modules of earlier tasks present | Holds |
| 10 | **Real source.** The Rust enumerator was run over every `.rs` file under `crates/` of this repository (169 test ids, no error) and its names for `fleet-core`, `harness-protocol` and `harness-conformance` were compared with `cargo test -- --list` (68 names, identical). The JavaScript enumerator was run over the 23 vitest files under `cockpit/ui/src` (166 ids, no error) | As stated. This supports assumption A4 only |
| 11 | **Real report.** `node --test --test-reporter=junit` was run on this plan's `cart.test.js` with Node 22.17.1 | Nesting and `classname="test"` as assumed (A6); **no `file` attribute**, contradicting A7 for that Node version |
| 12 | **Public-repository rule.** No sentence of the private design or of the doctrine is quoted; requirements are restated; doctrine is cited by id (C1) only | Doc comments and prose were compared with the private documents' wording and reworded wherever they echoed it |
| 13 | **Coverage of the brief.** Spec: parsing, validation, split, scope and overlap, `child_view`, exact-byte hash, testkit, vectors. Presets: report reading, id enumeration, marker, protected patterns, test regions, manifest reading, secret rules, testkit, vectors, the reconcile task | Each has a task |

**Gaps that could not be closed here.**

- **G1. The report shapes are unverified** (assumptions A1, A2, A5, A8), and one is already known to be false on one Node version (A7). cargo-nextest is not installed on the machine this was written on and vitest was not run. Task 25 exists for this.
- **G2. A cargo test id needs the package name, and `enumerate_ids(path, source)` cannot see it** (A3). The plan takes it from the directory name and refuses a root-level package. This is a hole in the shared interface, not something a fixture can fix (Q4).
- **G3. Out-of-line Rust test modules are invisible to `test_regions`** unless the file says `#![cfg(test)]` (Q8). The protected-tests control therefore cannot rely on `test_regions` alone for such files.
- **G4. The source scanners are not parsers.** They were exercised on this repository's 169 Rust tests and 166 vitest tests without an error, which is evidence, not proof. Their failure modes are listed at the top of the presets lane; all but G3 fail closed.
- **G5. Unicode normalisation of paths.** Paths compare case-insensitively by Unicode lower-casing; two spellings of one name that differ only in composed versus decomposed form compare unequal. Not handled; neither dependency list allows a normalisation crate.
- **G6. The replay ran on Windows only.** Linux is covered by CI when the lanes open their pull requests, not by this plan.
- **G7. `factory/m0` does not exist yet** on `origin` as this is written, so the worktree commands and the skeleton check in Tasks 1 and 11 were not run against the real branch.

**Open questions for the owner.**

- **Q1.** A split that lists exactly one child: this plan refuses it (`SplitTooSmall`, D4). The other reading is to ignore the split and run one unit. Which is wanted?
- **Q2.** Criterion ids are fixed to `AC-<n>` and markers to `ac<n>`. Is any other prefix needed?
- **Q3.** `work_item` in the header is an opaque string (`owner/repo#12`). Should it be the structured work item the protocol carries?
- **Q4.** Should `enumerate_ids` receive the package name (or the manifest), or should a cargo test id leave the package out?
- **Q5.** `node --test` on Node 22.17.1 writes no `file` attribute. Pin a Node release that does, or produce the report another way? And if the attribute is an absolute path, where does the root come from?
- **Q6.** Vitest reports paths relative to its own root. For a repository whose vitest root is a subdirectory, should the configuration name that root, or the reporter be configured to write repository-relative paths?
- **Q7.** A test that passed only on a retry is counted as not passed (D14). Is that the wanted strictness?
- **Q8.** Should out-of-line test modules be forbidden in onboarded cargo repositories, required to start with `#![cfg(test)]`, or found some other way?
- **Q9.** Nothing checks that a `bugfix` spec has a reproduction, or that `forms_affected` names Forms that exist. Both are outside the listed refusals and were left out. Add them?
