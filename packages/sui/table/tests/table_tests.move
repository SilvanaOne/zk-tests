#[test_only]
module table::table_tests;

use table::table;

const ENotImplemented: u64 = 0;

#[test]
fun test_table() {}

#[test, expected_failure(abort_code = ::table::table_tests::ENotImplemented)]
fun test_table_fail() {
    abort ENotImplemented
}
