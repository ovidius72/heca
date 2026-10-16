use super::*;

/// **One reading of an exit code**: `0` is success, anything else an error — for the shell's
/// report of a finished command, the process ending, and the close policy alike.
#[test]
fn an_exit_code_reads_as_success_only_when_it_is_zero() {
    assert_eq!(ProcessStatus::of_exit(0), ProcessStatus::Success);
    assert_eq!(ProcessStatus::of_exit(1), ProcessStatus::Error);
    assert_eq!(ProcessStatus::of_exit(-1), ProcessStatus::Error);
}

/// The close policy keeps a pane on the same reading: an error keeps it when asked to, a success
/// keeps it when asked to.
#[test]
fn the_close_policy_reads_exit_codes_the_same_way() {
    let keep_errors = PaneClosePolicy {
        close_pane: true,
        keep_on_error: true,
        keep_on_success: false,
    };
    assert!(!keep_errors.should_close(Some(2)));
    assert!(keep_errors.should_close(Some(0)));
    let keep_successes = PaneClosePolicy {
        close_pane: true,
        keep_on_error: false,
        keep_on_success: true,
    };
    assert!(!keep_successes.should_close(Some(0)));
    assert!(keep_successes.should_close(Some(2)));
}
