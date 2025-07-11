#[test_only]
module commitment::exp_tests;

use commitment::constants::get_r;
use commitment::exp::{r_scalar_pow, r_scalar_pow_legacy};
use std::debug;
use sui::bls12381::{Scalar, scalar_mul, scalar_from_u64};
use sui::group_ops::Element;

#[test_only]
use sui::test_scenario as test;
#[test_only]
use sui::random;

/// Naive scalar exponentiation for comparison (similar to polynomial.move's scalar_pow)
fun scalar_pow(base: &Element<Scalar>, exp: u64): Element<Scalar> {
    let mut acc = scalar_from_u64(1); // Start with 1
    let mut i = 0;
    while (i < exp) {
        acc = scalar_mul(&acc, base);
        i = i + 1;
    };
    acc
}

/// Helper function to compare two scalar values and print debug info only on failure
fun compare_and_debug_on_error(
    optimized_result: &Element<Scalar>,
    naive_result: &Element<Scalar>,
    exp: u64,
    test_number: u64,
): bool {
    let are_equal = *optimized_result == *naive_result;

    if (!are_equal) {
        debug::print(&std::string::utf8(b"❌ FAIL - Test "));
        debug::print(&test_number);
        debug::print(&std::string::utf8(b" - Exponent: "));
        debug::print(&exp);
        debug::print(&std::string::utf8(b"Optimized result: "));
        debug::print(optimized_result);
        debug::print(&std::string::utf8(b"Naive result: "));
        debug::print(naive_result);
    };

    are_equal
}

#[test]
fun test_r_scalar_pow_vs_naive_random_exponents() {
    debug::print(
        &std::string::utf8(
            b"Testing optimized r_scalar_pow vs naive scalar_pow",
        ),
    );

    let alice: address = @0xa11ce;
    let scenario = test::begin(alice);

    // Create random generator
    let mut rng = random::new_generator_for_testing();
    let r = get_r();

    debug::print(&std::string::utf8(b"Running 100 random tests..."));

    let mut test_count = 0;
    let mut all_passed = true;
    let mut failed_count = 0;

    while (test_count < 100) {
        // Generate random exponent in reasonable range
        // Use smaller range to avoid timeout with naive implementation
        let exp = random::generate_u64_in_range(&mut rng, 0, 5000);

        // Call both functions
        let optimized_result = r_scalar_pow(exp);
        let naive_result = scalar_pow(&r, exp);

        // Compare and debug only on error
        let test_passed = compare_and_debug_on_error(
            &optimized_result,
            &naive_result,
            exp,
            test_count + 1,
        );

        if (!test_passed) {
            all_passed = false;
            failed_count = failed_count + 1;
        };

        test_count = test_count + 1;
    };

    debug::print(&std::string::utf8(b"=== FINAL RESULT ==="));
    debug::print(&std::string::utf8(b"Total tests: 100"));
    debug::print(&std::string::utf8(b"Failed tests: "));
    debug::print(&failed_count);

    if (all_passed) {
        debug::print(
            &std::string::utf8(
                b"🎉 ALL 100 TESTS PASSED! Optimized function works correctly.",
            ),
        );
    } else {
        debug::print(
            &std::string::utf8(
                b"💥 SOME TESTS FAILED! There's a bug in the optimized function.",
            ),
        );
    };

    test::end(scenario);

    // Fail the test if any comparison failed
    assert!(all_passed, 999); // Custom error code for test failure
}

#[test]
fun test_r_scalar_pow_vs_legacy_random_exponents() {
    debug::print(
        &std::string::utf8(
            b"Testing optimized r_scalar_pow vs legacy r_scalar_pow_legacy",
        ),
    );

    let alice: address = @0xa11ce;
    let scenario = test::begin(alice);

    // Create random generator
    let mut rng = random::new_generator_for_testing();

    debug::print(
        &std::string::utf8(
            b"Running 100 random tests with smaller exponents...",
        ),
    );

    let mut test_count = 0;
    let mut all_passed = true;
    let mut failed_count = 0;

    while (test_count < 100) {
        // Generate smaller random exponent for legacy function (to avoid timeout)
        let exp = random::generate_u64_in_range(&mut rng, 0, 1000);

        // Call both functions
        let optimized_result = r_scalar_pow(exp);
        let legacy_result = r_scalar_pow_legacy(exp);

        // Compare and debug only on error
        let test_passed = compare_and_debug_on_error(
            &optimized_result,
            &legacy_result,
            exp,
            test_count + 1,
        );

        if (!test_passed) {
            all_passed = false;
            failed_count = failed_count + 1;
        };

        test_count = test_count + 1;
    };

    debug::print(&std::string::utf8(b"=== FINAL RESULT ==="));
    debug::print(&std::string::utf8(b"Total tests: 100"));
    debug::print(&std::string::utf8(b"Failed tests: "));
    debug::print(&failed_count);

    if (all_passed) {
        debug::print(
            &std::string::utf8(
                b"🎉 ALL 100 TESTS PASSED! Optimized function matches legacy.",
            ),
        );
    } else {
        debug::print(
            &std::string::utf8(
                b"💥 SOME TESTS FAILED! Optimized function differs from legacy.",
            ),
        );
    };

    test::end(scenario);

    // Fail the test if any comparison failed
    assert!(all_passed, 998); // Custom error code for test failure
}

#[test]
fun test_r_scalar_pow_large_random_exponents() {
    debug::print(
        &std::string::utf8(
            b"Testing optimized r_scalar_pow with large random exponents",
        ),
    );

    let alice: address = @0xa11ce;
    let scenario = test::begin(alice);

    // Create random generator
    let mut rng = random::new_generator_for_testing();

    debug::print(
        &std::string::utf8(
            b"Running 100 tests with large random exponents (up to 1M)...",
        ),
    );

    let mut test_count = 0;
    let mut all_passed = true;
    let mut failed_count = 0;

    while (test_count < 100) {
        // Generate large random exponent to test the optimized function's range
        let exp = random::generate_u64_in_range(&mut rng, 1000, 1000000);

        // Call optimized function
        let optimized_result = r_scalar_pow(exp);

        // For large exponents, we can't use naive comparison due to timeout
        // Instead, verify that the result is not zero and is a valid scalar
        let zero_scalar = scalar_from_u64(0);
        let test_passed = optimized_result != zero_scalar;

        if (!test_passed) {
            debug::print(&std::string::utf8(b"❌ FAIL - Test "));
            debug::print(&(test_count + 1));
            debug::print(&std::string::utf8(b" - Large exponent: "));
            debug::print(&exp);
            debug::print(&std::string::utf8(b"Got zero result!"));
            all_passed = false;
            failed_count = failed_count + 1;
        };

        test_count = test_count + 1;
    };

    debug::print(&std::string::utf8(b"=== FINAL RESULT ==="));
    debug::print(&std::string::utf8(b"Total large exponent tests: 100"));
    debug::print(&std::string::utf8(b"Failed tests: "));
    debug::print(&failed_count);

    if (all_passed) {
        debug::print(
            &std::string::utf8(b"🎉 ALL 100 LARGE EXPONENT TESTS PASSED!"),
        );
    } else {
        debug::print(
            &std::string::utf8(b"💥 SOME LARGE EXPONENT TESTS FAILED!"),
        );
    };

    test::end(scenario);

    // Fail the test if any comparison failed
    assert!(all_passed, 997); // Custom error code for test failure
}

#[test]
fun test_r_scalar_pow_specific_values() {
    debug::print(&std::string::utf8(b"Testing specific edge cases..."));

    let r = get_r();

    // Test exp = 0 (should return 1)
    let result_0 = r_scalar_pow(0);
    let expected_0 = scalar_from_u64(1);
    debug::print(&std::string::utf8(b"Testing exp = 0:"));
    debug::print(&result_0);
    assert!(result_0 == expected_0, 0);

    // Test exp = 1 (should return R)
    let result_1 = r_scalar_pow(1);
    let expected_1 = r;
    debug::print(&std::string::utf8(b"Testing exp = 1:"));
    debug::print(&result_1);
    assert!(result_1 == expected_1, 1);

    // Test exp = 2 (should return R²)
    let result_2 = r_scalar_pow(2);
    let expected_2 = scalar_mul(&r, &r);
    debug::print(&std::string::utf8(b"Testing exp = 2:"));
    debug::print(&result_2);
    assert!(result_2 == expected_2, 2);

    // Test some boundary values for our lookup tables
    let result_1023 = r_scalar_pow(1023); // Max index in TABLE0
    let expected_1023 = scalar_pow(&r, 1023);
    debug::print(&std::string::utf8(b"Testing exp = 1023:"));
    assert!(result_1023 == expected_1023, 1023);

    let result_1024 = r_scalar_pow(1024); // R^1024 (first entry in TABLE1)
    let expected_1024 = scalar_pow(&r, 1024);
    debug::print(&std::string::utf8(b"Testing exp = 1024:"));
    assert!(result_1024 == expected_1024, 1024);

    debug::print(&std::string::utf8(b"✅ All specific value tests passed!"));
}
