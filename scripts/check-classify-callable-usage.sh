#!/usr/bin/env bash
# INV-2 lint: Callable interface classification must use classify_callable_interface().
# Direct pattern matching on call_signatures for callable-ness detection is prohibited.
#
# Allowed patterns:
#   - collection.rs: classify_callable_interface definition
#   - Struct construction: `call_signatures: vec![]` or `call_signatures,`
#   - select_overload calls
#   - Test code
#
# Prohibited patterns:
#   - `call_signatures.is_empty()` or `!call_signatures.is_empty()` for callable detection
#     outside of classify_callable_interface
#
# Usage: ./scripts/check-classify-callable-usage.sh

set -euo pipefail

# Find callable-ness detection patterns (is_empty check on call_signatures)
# Exclude:
#   - collection.rs (classify_callable_interface definition lives here)
#   - test directories
#   - interfaces.rs (is_callable_only is AST-level, separate concern)

violations=$(
    grep -rn 'call_signatures.*is_empty\|call_signatures.*\.len()' src/ \
        --include='*.rs' \
        --exclude-dir='tests' \
    | grep -v 'collection\.rs' \
    | grep -v 'interfaces\.rs' \
    | grep -v '^\s*//' \
    || true
)

if [ -n "$violations" ]; then
    echo "WARNING: Direct call_signatures detection found outside classify_callable_interface."
    echo "These should migrate to classify_callable_interface() in Phase 4/9."
    echo ""
    echo "Violations:"
    echo "$violations"
    # Exit 0 for now — these are known pre-existing patterns that will be fixed
    # in Phase 4.1 (type_aliases.rs) and Phase 9.2 (helpers.rs).
    # Change to exit 1 after Phase 9 completion.
    exit 0
fi

echo "OK: No direct call_signatures detection outside classify_callable_interface."
