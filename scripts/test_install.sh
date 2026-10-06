#!/bin/zsh
# Run every install shell test. One PASS/FAIL line per script.
# Exit non-zero if any script fails.
set -euo pipefail

here=${0:A:h}
fail=0

run_one() {
  local name=$1
  if "$here/$name"; then
    print -r -- "PASS $name"
  else
    print -r -- "FAIL $name"
    fail=1
  fi
}

run_one test_install_lib.sh
run_one test_install_sidecar_config.sh
run_one test_check_bundled_ctl.sh
run_one test_build_tester_dmg.sh
exit $fail
