#[test_only]
module copytest::copy_tests;

use copytest::main;
use std::string;
use sui::test_scenario;
use sui::test_utils;

#[test]
fun test_create_main_object_and_add_nested_objects() {
    let mut scenario = test_scenario::begin(@0x123);
    let ctx = test_scenario::ctx(&mut scenario);

    // Create main object
    let title = string::utf8(b"Test Main Object");
    let mut numbers = std::vector::empty<u256>();
    std::vector::push_back(&mut numbers, 42u256);
    std::vector::push_back(&mut numbers, 100u256);

    main::create_and_share_main_object(title, numbers, ctx);

    // Move to next transaction to access the shared object
    test_scenario::next_tx(&mut scenario, @0x123);

    // Get the shared main object
    let mut main_object = test_scenario::take_shared<main::MainObject>(
        &scenario,
    );

    // Verify initial state
    assert!(
        main::get_title(&main_object) == &string::utf8(b"Test Main Object"),
    );
    assert!(std::vector::length(main::get_numbers(&main_object)) == 2);
    assert!(main::get_nested_objects_count(&main_object) == 0);

    // Create and add first nested object
    let mut nested_values_1 = std::vector::empty<u256>();
    std::vector::push_back(&mut nested_values_1, 10u256);
    std::vector::push_back(&mut nested_values_1, 20u256);

    let ctx = test_scenario::ctx(&mut scenario);
    main::add_nested_object(
        &mut main_object,
        string::utf8(b"first_nested"),
        nested_values_1,
        string::utf8(b"First nested object for testing"),
        ctx,
    );

    // Create and add second nested object
    let mut nested_values_2 = std::vector::empty<u256>();
    std::vector::push_back(&mut nested_values_2, 30u256);
    std::vector::push_back(&mut nested_values_2, 40u256);
    std::vector::push_back(&mut nested_values_2, 50u256);

    main::add_nested_object(
        &mut main_object,
        string::utf8(b"second_nested"),
        nested_values_2,
        string::utf8(b"Second nested object for testing"),
        ctx,
    );

    // Verify both nested objects were added
    assert!(main::get_nested_objects_count(&main_object) == 2);
    assert!(
        main::contains_nested_object(
            &main_object,
            string::utf8(b"first_nested"),
        ),
    );
    assert!(
        main::contains_nested_object(
            &main_object,
            string::utf8(b"second_nested"),
        ),
    );

    // Test accessing nested objects
    let first_nested = main::get_nested_object(
        &main_object,
        string::utf8(b"first_nested"),
    );
    assert!(
        main::get_nested_description(first_nested) == &string::utf8(b"First nested object for testing"),
    );
    assert!(std::vector::length(main::get_nested_values(first_nested)) == 2);

    let second_nested = main::get_nested_object(
        &main_object,
        string::utf8(b"second_nested"),
    );
    assert!(
        main::get_nested_description(second_nested) == &string::utf8(b"Second nested object for testing"),
    );
    assert!(std::vector::length(main::get_nested_values(second_nested)) == 3);

    // Test updating nested object description
    main::update_nested_object_description(
        &mut main_object,
        string::utf8(b"first_nested"),
        string::utf8(b"Updated first nested object"),
    );

    let updated_first_nested = main::get_nested_object(
        &main_object,
        string::utf8(b"first_nested"),
    );
    assert!(
        main::get_nested_description(updated_first_nested) == &string::utf8(b"Updated first nested object"),
    );

    // Test adding values to nested objects
    main::add_value_to_nested_object(
        &mut main_object,
        string::utf8(b"first_nested"),
        99u256,
    );

    let first_nested_after_add = main::get_nested_object(
        &main_object,
        string::utf8(b"first_nested"),
    );
    assert!(
        std::vector::length(main::get_nested_values(first_nested_after_add)) == 3,
    );

    // Return the shared object
    test_scenario::return_shared(main_object);
    test_scenario::end(scenario);
}

#[test]
fun test_main_object_operations() {
    let mut scenario = test_scenario::begin(@0x456);
    let ctx = test_scenario::ctx(&mut scenario);

    // Create main object with empty numbers
    let title = string::utf8(b"Operations Test");
    let numbers = std::vector::empty<u256>();

    main::create_and_share_main_object(title, numbers, ctx);

    // Move to next transaction
    test_scenario::next_tx(&mut scenario, @0x456);

    // Get the shared main object
    let mut main_object = test_scenario::take_shared<main::MainObject>(
        &scenario,
    );

    // Test updating title
    main::update_title(&mut main_object, string::utf8(b"Updated Title"));
    assert!(main::get_title(&main_object) == &string::utf8(b"Updated Title"));

    // Test adding numbers
    main::add_number(&mut main_object, 123u256);
    main::add_number(&mut main_object, 456u256);

    assert!(std::vector::length(main::get_numbers(&main_object)) == 2);

    // Test empty table
    assert!(main::is_nested_objects_empty(&main_object));

    // Add one nested object
    let ctx = test_scenario::ctx(&mut scenario);
    main::add_nested_object(
        &mut main_object,
        string::utf8(b"test_key"),
        std::vector::singleton(777u256),
        string::utf8(b"Test nested object"),
        ctx,
    );

    // Test table is no longer empty
    assert!(!main::is_nested_objects_empty(&main_object));

    // Test removing nested object
    let removed_nested = main::remove_nested_object(
        &mut main_object,
        string::utf8(b"test_key"),
    );
    assert!(
        main::get_nested_description(&removed_nested) == &string::utf8(b"Test nested object"),
    );

    // Test table is empty again
    assert!(main::is_nested_objects_empty(&main_object));

    // Clean up removed object
    test_utils::destroy(removed_nested);

    // Return the shared object
    test_scenario::return_shared(main_object);
    test_scenario::end(scenario);
}

#[test]
fun test_create_sample_object() {
    let mut scenario = test_scenario::begin(@0x789);
    let ctx = test_scenario::ctx(&mut scenario);

    // Create sample object
    main::create_sample_object(ctx);

    // Move to next transaction
    test_scenario::next_tx(&mut scenario, @0x789);

    // Get the shared main object
    let main_object = test_scenario::take_shared<main::MainObject>(&scenario);

    // Verify sample object was created correctly
    assert!(
        main::get_title(&main_object) == &string::utf8(b"Sample Main Object"),
    );
    assert!(std::vector::length(main::get_numbers(&main_object)) == 3);
    assert!(main::get_nested_objects_count(&main_object) == 2);

    // Verify nested objects exist
    assert!(main::contains_nested_object(&main_object, string::utf8(b"first")));
    assert!(
        main::contains_nested_object(&main_object, string::utf8(b"second")),
    );

    // Check first nested object
    let first_nested = main::get_nested_object(
        &main_object,
        string::utf8(b"first"),
    );
    assert!(
        main::get_nested_description(first_nested) == &string::utf8(b"First nested object"),
    );
    assert!(std::vector::length(main::get_nested_values(first_nested)) == 2);

    // Check second nested object
    let second_nested = main::get_nested_object(
        &main_object,
        string::utf8(b"second"),
    );
    assert!(
        main::get_nested_description(second_nested) == &string::utf8(b"Second nested object"),
    );
    assert!(std::vector::length(main::get_nested_values(second_nested)) == 3);

    // Return the shared object
    test_scenario::return_shared(main_object);
    test_scenario::end(scenario);
}

#[test]
#[expected_failure]
fun test_get_nonexistent_nested_object() {
    let mut scenario = test_scenario::begin(@0x999);
    let ctx = test_scenario::ctx(&mut scenario);

    // Create main object
    main::create_and_share_main_object(
        string::utf8(b"Test"),
        std::vector::empty<u256>(),
        ctx,
    );

    // Move to next transaction
    test_scenario::next_tx(&mut scenario, @0x999);

    // Get the shared main object
    let main_object = test_scenario::take_shared<main::MainObject>(&scenario);

    // This should fail - trying to get a non-existent nested object
    let _nonexistent = main::get_nested_object(
        &main_object,
        string::utf8(b"nonexistent"),
    );

    // Return the shared object
    test_scenario::return_shared(main_object);
    test_scenario::end(scenario);
}

#[test]
fun test_copy_object() {
    let mut scenario = test_scenario::begin(@0xABC);
    let ctx = test_scenario::ctx(&mut scenario);

    // Create main object
    let title = string::utf8(b"Original Object");
    let mut numbers = std::vector::empty<u256>();
    std::vector::push_back(&mut numbers, 111u256);
    std::vector::push_back(&mut numbers, 222u256);
    std::vector::push_back(&mut numbers, 333u256);

    main::create_and_share_main_object(title, numbers, ctx);

    // Move to next transaction to access the shared object
    test_scenario::next_tx(&mut scenario, @0xABC);

    // Get the shared main object
    let mut main_object = test_scenario::take_shared<main::MainObject>(
        &scenario,
    );

    // Add nested objects to the original
    let ctx = test_scenario::ctx(&mut scenario);
    main::add_nested_object(
        &mut main_object,
        string::utf8(b"nested1"),
        std::vector::singleton(100u256),
        string::utf8(b"First nested"),
        ctx,
    );

    main::add_nested_object(
        &mut main_object,
        string::utf8(b"nested2"),
        vector[200u256, 300u256],
        string::utf8(b"Second nested"),
        ctx,
    );

    // Get the current version
    let current_version = main::get_version(&main_object);
    assert!(current_version == 3); // Started at 1, added 2 nested objects

    // Test copying with correct version
    let keys_to_copy = vector[
        string::utf8(b"nested1"),
        string::utf8(b"nested2"),
    ];
    let copied_object = main::copy_object(
        &main_object,
        keys_to_copy,
        current_version,
        ctx,
    );

    // Verify the copied object has the same data
    assert!(main::get_title(&copied_object) == main::get_title(&main_object));
    assert!(
        main::get_numbers(&copied_object) == main::get_numbers(&main_object),
    );
    assert!(
        main::get_version(&copied_object) == main::get_version(&main_object),
    );
    assert!(main::get_nested_objects_count(&copied_object) == 2);

    // Verify nested objects were copied
    assert!(
        main::contains_nested_object(&copied_object, string::utf8(b"nested1")),
    );
    assert!(
        main::contains_nested_object(&copied_object, string::utf8(b"nested2")),
    );

    // Check nested object contents
    let original_nested1 = main::get_nested_object(
        &main_object,
        string::utf8(b"nested1"),
    );
    let copied_nested1 = main::get_nested_object(
        &copied_object,
        string::utf8(b"nested1"),
    );
    assert!(
        main::get_nested_description(original_nested1) == main::get_nested_description(copied_nested1),
    );
    assert!(
        main::get_nested_values(original_nested1) == main::get_nested_values(copied_nested1),
    );

    let original_nested2 = main::get_nested_object(
        &main_object,
        string::utf8(b"nested2"),
    );
    let copied_nested2 = main::get_nested_object(
        &copied_object,
        string::utf8(b"nested2"),
    );
    assert!(
        main::get_nested_description(original_nested2) == main::get_nested_description(copied_nested2),
    );
    assert!(
        main::get_nested_values(original_nested2) == main::get_nested_values(copied_nested2),
    );

    // Test copying with partial keys
    let partial_keys = vector[string::utf8(b"nested1")];
    let partial_copied = main::copy_object(
        &main_object,
        partial_keys,
        current_version,
        ctx,
    );
    assert!(main::get_nested_objects_count(&partial_copied) == 1);
    assert!(
        main::contains_nested_object(&partial_copied, string::utf8(b"nested1")),
    );
    assert!(
        !main::contains_nested_object(
            &partial_copied,
            string::utf8(b"nested2"),
        ),
    );

    // Test copying with empty keys
    let empty_keys = vector[];
    let empty_copied = main::copy_object(
        &main_object,
        empty_keys,
        current_version,
        ctx,
    );
    assert!(main::get_nested_objects_count(&empty_copied) == 0);
    assert!(main::is_nested_objects_empty(&empty_copied));

    // Clean up
    test_utils::destroy(copied_object);
    test_utils::destroy(partial_copied);
    test_utils::destroy(empty_copied);

    // Return the shared object
    test_scenario::return_shared(main_object);
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = copytest::main::EInvalidVersion)]
fun test_copy_object_invalid_version() {
    let mut scenario = test_scenario::begin(@0xDEF);
    let ctx = test_scenario::ctx(&mut scenario);

    // Create main object
    main::create_and_share_main_object(
        string::utf8(b"Test Object"),
        std::vector::singleton(456u256),
        ctx,
    );

    // Move to next transaction
    test_scenario::next_tx(&mut scenario, @0xDEF);

    // Get the shared main object
    let mut main_object = test_scenario::take_shared<main::MainObject>(
        &scenario,
    );

    // Add a nested object to increment version
    let ctx = test_scenario::ctx(&mut scenario);
    main::add_nested_object(
        &mut main_object,
        string::utf8(b"test"),
        std::vector::singleton(789u256),
        string::utf8(b"Test nested"),
        ctx,
    );

    // Try to copy with wrong version - this should fail
    let keys = vector[string::utf8(b"test")];
    let wrong_version = 1; // Object version is now 2
    let copied = main::copy_object(&main_object, keys, wrong_version, ctx);

    // This won't execute due to abort, but satisfies Move's type checker
    test_utils::destroy(copied);
    test_scenario::return_shared(main_object);
    test_scenario::end(scenario);
}

#[test]
fun test_copy_object_basic() {
    let mut scenario = test_scenario::begin(@0x111);
    let ctx = test_scenario::ctx(&mut scenario);

    // Create main object
    main::create_and_share_main_object(
        string::utf8(b"Basic Copy Test"),
        vector[10u256, 20u256],
        ctx,
    );

    // Move to next transaction
    test_scenario::next_tx(&mut scenario, @0x111);

    // Get the shared main object
    let mut main_object = test_scenario::take_shared<main::MainObject>(
        &scenario,
    );

    // Add nested objects
    let ctx = test_scenario::ctx(&mut scenario);
    main::add_nested_object(
        &mut main_object,
        string::utf8(b"will_not_copy"),
        std::vector::singleton(999u256),
        string::utf8(b"This won't be copied"),
        ctx,
    );

    // Use copy_object_basic which doesn't copy nested objects
    let basic_copy = main::copy_object_basic(&main_object, ctx);

    // Verify basic fields are copied but nested objects table is empty
    assert!(main::get_title(&basic_copy) == main::get_title(&main_object));
    assert!(main::get_numbers(&basic_copy) == main::get_numbers(&main_object));
    assert!(main::get_version(&basic_copy) == main::get_version(&main_object));
    assert!(main::is_nested_objects_empty(&basic_copy));

    // Clean up
    test_utils::destroy(basic_copy);

    // Return the shared object
    test_scenario::return_shared(main_object);
    test_scenario::end(scenario);
}
