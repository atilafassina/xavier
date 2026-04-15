# Multi-Model Debate Protocol

The debate protocol defines the contract between Xavier's review skill and a multi-model dispatch layer. When multiple models review the same diff, their findings are synthesized into a structured report that distinguishes agreement from disagreement and surfaces gaps in coverage.

## 1. Input Contract

Each debate call receives three inputs:

- **Diff**: The code change under review, in unified diff format.
- **Persona definition**: A Markdown file sent verbatim to each model (e.g., `correctness.md`, `security.md`, `performance.md`). The persona defines the reviewer's domain, what to look for, severity levels, and output format.
- **Filtered vault context**: Conventions and recurring patterns from the Xavier vault that are relevant to the persona's domain. Only context matching the persona's focus area is included — a security persona receives security-related conventions and past findings, not performance tuning notes.

Each model receives the same diff and the same persona definition. The vault context may be filtered differently per persona but is identical across models for the same persona.

## 2. Output Contract

The synthesis layer produces structured Markdown with three sections:

### `## Consensus`

Findings that both models independently flagged. These carry high confidence because two different models, reasoning independently, arrived at the same conclusion.

### `## Disputes`

Findings where the models disagree. Each dispute includes both sides: the model that flagged it and its reasoning, and the model that did not flag it (or flagged the opposite) and its reasoning. Disputes require human judgment to resolve.

### `## Blindspots`

Findings that only one model flagged and the other missed entirely. These are not disagreements — the second model simply did not consider the issue. Blindspots are valuable because they represent gaps in coverage that a single-model review would have missed.

### Finding Format

Every finding across all three sections must include:

- **Severity**: `critical`, `high`, `medium`, or `low`
- **File reference**: `file:line` pointing to the relevant code location
- **Description**: What the issue is and why it matters
- **Suggestion**: A concrete recommendation for how to address it

Example:

```markdown
### [high] Unbounded query in pagination handler

**File**: `src/api/users.ts:47`
**Description**: The offset-based query has no upper bound on page size, allowing a caller to request all rows in a single request.
**Suggestion**: Add a `MAX_PAGE_SIZE` constant and clamp the requested size before passing it to the query builder.
```

## 3. Consensus Threshold

With two models, consensus is binary: both agree or they do not. There is no partial consensus. The rules are:

- **Both models flag the same issue** -> Consensus
- **Models disagree on the same issue** -> Dispute
- **Only one model flags an issue** (the other is silent) -> Blindspot

If a third model is added in the future, the threshold should be redefined. Options include simple majority (2-of-3) or unanimous agreement, depending on the desired confidence level. This decision is deferred until a third model is actually integrated.

## 4. Vault Interaction Rules

The pilot fish (synthesis layer) has access to the Xavier vault and can adjust classifications based on historical patterns:

- **Upgrade**: When recurring patterns from the vault corroborate a Consensus finding, it is marked as "confirmed." This means not only do both models agree, but the vault's historical record supports the finding as a known pattern in this codebase.
- **Downgrade**: When recurring patterns from the vault contradict a Consensus finding, it is reclassified as a Dispute. Both models agree, but the vault's history disagrees — perhaps the pattern was previously reviewed and accepted as intentional, or the codebase has a documented convention that justifies the flagged code.

The pilot fish never creates new findings. It only reclassifies existing ones based on vault evidence.

## 5. Fallback Behavior

When the `agent` CLI is not found in `PATH`, the debate protocol is skipped entirely and Xavier falls back to the existing Claude-only three-persona review flow.

**Detection**: Run `which agent` during review pre-flight. If it exits non-zero, fall back.

**Requirements for fallback**:

- No error message is shown to the user. The absence of `agent` is a normal configuration state, not an error.
- No degraded debate mode. There is no "single-model debate" — either the full multi-model debate runs, or the existing Claude-only persona flow runs.
- The fallback produces a clean, complete review using the standard three-persona flow (correctness, security, performance), identical to what Xavier produced before multi-model support was added.
- The user sees no difference in output format between the fallback flow and a pre-debate-era review.

## 6. Persona Portability

Personas are designed to be model-agnostic. This is a deliberate design choice for v1.

**Rules**:

- Personas are sent verbatim as Markdown to every model. No transformation, no model-specific adaptation.
- Persona instructions must not contain model-specific language (e.g., "as Claude, you should..." or "as GPT, focus on..."). They describe WHAT to look for, not HOW a specific model should reason.
- Output format instructions (severity levels, `file:line` references, suggestion blocks) are universal. Every model is expected to produce findings in the same structure.
- If model-specific drift appears in practice (one model consistently misinterprets a persona instruction), an adapter layer can be introduced later to translate persona instructions per model. This adapter does not exist in v1.

This keeps personas as a single source of truth. Maintaining one persona file per domain is simpler and less error-prone than maintaining model-specific variants.

## 7. Dispatch Abstraction

The dispatch layer that actually sends prompts to external models lives as a dependency-skill at `deps/multi-model-dispatch/`, not inside this protocol.

This protocol defines **WHAT** the contract is: what inputs go in, what outputs come out, how findings are classified. The dependency-skill implements **HOW**: which providers to call, how to authenticate, how to handle rate limits and timeouts.

This separation enables future providers (ollama, openrouter, direct API calls) to be swapped in without changing the debate contract. A new provider only needs to satisfy the input/output contract defined here. The review skill consumes the contract; the dispatch skill fulfills it.
