#!/bin/sh
# The integration test scripts and their arguments, as the Makefile's
# test-launch, test-native-supervision, test-gate1-setup, test-tracee-kill,
# and test-process-native targets run them. Sourced by the runners that run
# them somewhere other than this machine. A test is named by its script:
# launch, gate1-setup, tracee-kill, process-native, and the native-supervision
# scripts by what follows `test-native-` (supervision, yield-broker, ...).

integration_races='yield-broker clone-snapshot clone-restart close-range-signal close-range-gap network-snapshot unix-send-snapshot'
integration_default="launch supervision $integration_races gate1-setup tracee-kill process-native"

# integration_command <test> <release dir> <launch dir> <gate1 dir> <process dir>
integration_command() {
    case $1 in
        launch)
            echo "python3 scripts/test-launch.py $2/ginux $3/launch-fixture" ;;
        gate1-setup)
            echo "python3 scripts/test-gate1-setup.py $2/ginux $4/deny-ptrace $4/payload" ;;
        tracee-kill)
            echo "python3 scripts/test-tracee-kill.py $2/ginux $4/deny-ptrace $4/payload $2/managed-payload $3/native-fixture" ;;
        process-native)
            echo "python3 scripts/test-process-native.py $2/ginux $5 $2/process-native-fixture" ;;
        supervision)
            echo "python3 scripts/test-native-supervision.py $2/ginux $3/native-fixture" ;;
        page-size)
            # aarch64 only: see tests/fixtures/page-size-fixture.c.
            echo "python3 scripts/test-native-page-size.py $2/ginux $3/page-size-fixture" ;;
        clone-snapshot)
            echo "python3 scripts/aarch64-trace.py scripts/test-native-clone-snapshot.py $2/ginux $3/native-fixture $3/native-clone-collision-fixture" ;;
        *)
            # The rest trace the daemon and read its registers as x86-64.
            # aarch64-trace.py gives them that view; elsewhere it only runs
            # the script.
            if [ ! -f "scripts/test-native-$1.py" ]; then
                echo "unknown integration test: $1" >&2
                return 2
            fi
            echo "python3 scripts/aarch64-trace.py scripts/test-native-$1.py $2/ginux $3/native-fixture" ;;
    esac
}

# How a line that a passing test prints begins. The race scripts print only
# their checks.
integration_pass_line() {
    case $1 in
        launch|gate1-setup|tracee-kill|process-native) echo "test-$1: PASS" ;;
        supervision) echo "test-native-supervision: PASS" ;;
        *) echo "PASS native " ;;
    esac
}
