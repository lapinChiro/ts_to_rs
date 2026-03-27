---
name: investigation
description: Investigation procedure when user requests research/analysis. Thoroughly read source code, docs, and web resources, saving reports to report/
user-invocable: true
---

# Investigation Task Execution

## Trigger

When the user requests an investigation or analysis.

## Actions

1. **Thoroughly** read all relevant source code, documentation, and web resources
   - Source code: Read related modules in full (do not settle for partial reads)
   - Documentation: Check README, CLAUDE.md, TODO, plan.md, backlog/
   - External resources: Perform web searches and document retrieval as needed
2. Save results to `report/<theme-name>.md`
   - Use kebab-case theme names that indicate the investigation content (e.g., `report/design-issues.md`, `report/swc-api-changes.md`)
   - Include the following metadata at the top of the report:
     - **Base commit**: Output of `git rev-parse --short HEAD` (to identify the codebase at investigation time)
     - If investigating with uncommitted changes, note this
   - Include a summary, detailed analysis, and references to supporting code locations/documentation
3. Report a summary of findings to the user

## Prohibited

- Reading only some files and reporting "confirmed the whole thing"
- Composing reports from speculation or generalities only (support with specific code locations, file names, and line numbers)
- Reporting verbally only without creating a report file
- Creating a report without documenting the base commit
