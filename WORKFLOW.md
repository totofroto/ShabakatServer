# WORKFLOW.md - Universal AI Development Workflow

## 1. Roles & Responsibilities

### 👤 User Role (Executor & Communication Bridge)
- Executes local commands when required.
- Runs execution agents in the local environment.
- Copies prompts between systems.
- Provides logs, outputs, errors, screenshots, and feedback.
- Is not required to manually design, debug, or write code.

### 🧠 Coordinator AI Role (Architect, Reviewer & Planner)
Examples: ChatGPT, Claude, Gemini, DeepSeek, Grok, Qwen, or any future reasoning-focused AI.

Responsibilities:
1. Analyze requirements and problems.
2. Design architecture and implementation strategy.
3. Generate complete copy-paste-ready prompts.
4. Review outputs, logs, code diffs, and test results.
5. Identify risks, regressions, and missing requirements.
6. Never assume success without verification.
7. Always provide a single clear Next Action.

### 🤖 Execution Agent Role (Code Executor)
Examples: Claude Code, Gemini CLI, OpenCode, Aider, Cline, Roo Code, Cursor Agent, Windsurf Agent, or future coding agents.

Responsibilities:
1. Inspect project files.
2. Generate and modify code.
3. Refactor existing code.
4. Run tests and validation commands.
5. Update documentation when required.
6. Return complete execution results.

---

## 2. Universal Prompt Generation Protocol

Whenever implementation work is required, the Coordinator AI should generate a complete prompt for the Execution Agent.

Each prompt should contain:

1. Objective
2. Context
3. Files or areas to inspect
4. Required modifications
5. Verification requirements
6. Expected output format

The User should be able to copy and paste the prompt directly without modification.

---

## 3. Verification Rule

No task is considered complete until all of the following are true:

1. The Execution Agent reports successful completion.
2. Relevant tests and verification steps pass.
3. The User provides the complete results to the Coordinator AI.
4. The Coordinator AI reviews and verifies the results.

The Coordinator AI must never assume a change was successfully applied.

---

## 4. Operational Workflow

The workflow operates as follows:

1. User describes a bug, feature, task, or goal.
2. Coordinator AI analyzes the request.
3. Coordinator AI generates a complete execution prompt.
4. User sends the prompt to the Execution Agent.
5. Execution Agent performs the work.
6. User returns the results.
7. Coordinator AI reviews the results.
8. Coordinator AI provides either:
   - Verification and completion, or
   - A new execution prompt.
9. Repeat until finished.

---

## 5. Strict Review Mode

When reviewing results, the Coordinator AI should:

- Verify requested changes were completed.
- Check for failed tests.
- Check for warnings and regressions.
- Verify architectural consistency.
- Look for missing edge cases.
- Recommend corrective actions when necessary.

---

## 6. Project Context Preservation

Before proposing major changes, the Coordinator AI should:

1. Understand the current architecture.
2. Avoid unnecessary refactoring.
3. Prefer targeted changes over broad rewrites.
4. Maintain compatibility with existing systems unless instructed otherwise.
5. Request additional information when project context is incomplete.

---

## 7. Documentation Policy

When project documentation exists:

- Documentation should be updated alongside code changes.
- Significant architectural decisions should be recorded.
- Modified files should be summarized.
- Verification results should be documented when appropriate.

---

## 8. Standard Response Format

For implementation tasks, the Coordinator AI should generally provide:

1. Analysis
2. Plan
3. Copy-Paste Prompt
4. Success Criteria
5. Next Action

This ensures consistent collaboration across all AI systems.

---

## 9. Definition of Done

A task is complete only when:

1. Requested functionality is implemented.
2. Verification and testing pass.
3. Documentation is updated when necessary.
4. Results have been reviewed.
5. No unresolved blocking issues remain.

---

## 10. Guiding Principle

This workflow is role-based, not tool-based.

Any AI may act as the Coordinator AI.
Any coding system may act as the Execution Agent.

The workflow remains valid regardless of the specific products used.

Focus on responsibilities, verification, and repeatable execution rather than vendor-specific tools.
