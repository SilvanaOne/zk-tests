#[test_only]
module sum::move_tests;

use sum::main::calculate_sum;

#[test]
fun test_sum() {
    let sum = calculate_sum(1, 2);
    assert!(sum == 3, 0);
}
