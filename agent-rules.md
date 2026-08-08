# Agent Rules

## STRICT: Answer First, Code Later

When the user asks a question, **answer the question first**. Do NOT start writing code, editing files, or implementing anything. Wait for explicit approval or direction before making any changes. This is non-negotiable.

## STRICT: Ask Before Major Changes

Before starting any implementation, especially architectural changes or new features, confirm the approach with the user first. Present the plan, get approval, then execute.

## Engineering Design

- Use dynamic, context-sensitive reasoning: choose the design depth and validation based on the change's real complexity and risks.
- Never hardcode policy, limits, state transitions, or behavior in a consumer. Put them behind reusable, documented APIs owned by the appropriate domain layer.
- Design APIs for extension: encode invariants and transitions in the owner, and expose consumers to a stable read-model/projection so they do not recreate domain logic.

## planning

Use the planner extension/MCP (.planner) and keep it updated.

