---
name: backlog-replenishment
description: Replenishment procedure when backlog/ is empty and user requests work. Identify PRD-eligible TODO items, then create PRDs through Discovery
user-invocable: true
---

# Backlog Replenishment

## Trigger

When `backlog/` is empty and the user requests work.

## Actions

1. Review `TODO`
2. Assess all items for PRD eligibility (criteria below)
3. For items with hold reasons, self-evaluate resolution likelihood and select up to 2 with highest probability:
   - If the hold reason is "waiting for another implementation to complete", check the codebase and tests yourself. No user confirmation needed
   - User confirmation is only needed when the prerequisite involves external decisions or user experience that you cannot judge yourself
4. For items with hold reasons you cannot self-resolve, confirm with the user one at a time (do not batch)
5. Include items confirmed/determined as resolved in PRD candidates
6. Follow the PRD template (/prd-template): Discovery → PRD drafting
7. Place the created PRD in `backlog/` and delete the corresponding item from `TODO`
8. Insert the new item into `plan.md` execution order

### PRD Eligibility Criteria

Assess each TODO item against these criteria:

**PRD-eligible:**
- Items with no stated hold reason
- Items whose only hold reason is "needs design", "needs investigation", or "needs decision" — these are resolved during the PRD's Discovery phase. Design, investigation, and decisions are part of the PRD process, not prerequisites for PRD creation

**Not PRD-eligible (legitimate hold reasons):**
- Another feature/PRD is a prerequisite ("start after X is implemented")
- Real operational data/results are needed ("decide after seeing usage in real projects")
- Waiting on external decisions ("waiting for user's policy decision")

**Judgment principle:**
The criterion is "Can we start Discovery (clarification questions) for this item?" If Discovery can start, it's PRD-eligible. If another task must complete before Discovery, it's not PRD-eligible.

## Prohibited

- Judging "not PRD-eligible" without verifying hold reason validity (check implementation status yourself; confirm external factors with user)
- Batching hold reason confirmations (confirm one at a time)
- Creating a PRD for items whose hold reasons the user confirmed as unresolved
- Skipping Discovery (clarification questions) when writing a PRD
- Forgetting to delete the corresponding TODO item after PRD creation
- Forgetting to insert into `plan.md` after PRD creation
- Treating "needs design" or "needs investigation" as hold reasons — these should be resolved in Discovery and do not block PRD creation
- Inferring implicit reasons and judging items without stated hold reasons as "not PRD-eligible"
- Judging as not PRD-eligible because "Rust has no direct syntax equivalent" — if no conversion method is found, interview the user
