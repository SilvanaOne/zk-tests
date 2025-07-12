module copytest::main;

use std::string::{Self, String};
use sui::object_table::{Self, ObjectTable};

// For Move coding conventions, see
// https://docs.sui.io/concepts/sui-move-concepts/conventions

/// Nested object that goes into the table
public struct NestedObject has key, store {
    id: sui::object::UID,
    values: vector<u256>,
    description: String,
}

/// Main object structure
public struct MainObject has key {
    id: sui::object::UID,
    title: String,
    numbers: vector<u256>,
    nested_objects: ObjectTable<String, NestedObject>,
    version: u64,
}

/// Create a new nested object
public fun new_nested_object(
    values: vector<u256>,
    description: String,
    ctx: &mut sui::tx_context::TxContext,
): NestedObject {
    NestedObject {
        id: sui::object::new(ctx),
        values,
        description,
    }
}

/// Create and share the main object
public fun create_and_share_main_object(
    title: String,
    numbers: vector<u256>,
    ctx: &mut sui::tx_context::TxContext,
) {
    let main_object = MainObject {
        id: sui::object::new(ctx),
        title,
        numbers,
        nested_objects: object_table::new(ctx),
        version: 1,
    };

    sui::transfer::share_object(main_object);
}

/// Add a nested object to the main object's table
public fun add_nested_object(
    main_object: &mut MainObject,
    key: String,
    values: vector<u256>,
    description: String,
    ctx: &mut sui::tx_context::TxContext,
) {
    let nested_obj = new_nested_object(values, description, ctx);
    main_object.nested_objects.add(key, nested_obj);
    main_object.version = main_object.version + 1;
}

/// Get the title of the main object
public fun get_title(main_object: &MainObject): &String {
    &main_object.title
}

/// Get the numbers vector from the main object
public fun get_numbers(main_object: &MainObject): &vector<u256> {
    &main_object.numbers
}

/// Get the version of the main object
public fun get_version(main_object: &MainObject): u64 {
    main_object.version
}

/// Get a nested object from the table
public fun get_nested_object(
    main_object: &MainObject,
    key: String,
): &NestedObject {
    main_object.nested_objects.borrow(key)
}

/// Get a mutable reference to a nested object from the table
public fun get_nested_object_mut(
    main_object: &mut MainObject,
    key: String,
): &mut NestedObject {
    main_object.nested_objects.borrow_mut(key)
}

/// Get the description from a nested object
public fun get_nested_description(nested_obj: &NestedObject): &String {
    &nested_obj.description
}

/// Get the values vector from a nested object
public fun get_nested_values(nested_obj: &NestedObject): &vector<u256> {
    &nested_obj.values
}

/// Update the title of the main object
public fun update_title(main_object: &mut MainObject, new_title: String) {
    main_object.title = new_title;
    main_object.version = main_object.version + 1;
}

/// Add a number to the main object's numbers vector
public fun add_number(main_object: &mut MainObject, number: u256) {
    std::vector::push_back(&mut main_object.numbers, number);
    main_object.version = main_object.version + 1;
}

/// Remove a nested object from the table
public fun remove_nested_object(
    main_object: &mut MainObject,
    key: String,
): NestedObject {
    main_object.version = main_object.version + 1;
    main_object.nested_objects.remove(key)
}

/// Check if a key exists in the nested objects table
public fun contains_nested_object(main_object: &MainObject, key: String): bool {
    main_object.nested_objects.contains(key)
}

/// Get the size of the nested objects table
public fun get_nested_objects_count(main_object: &MainObject): u64 {
    main_object.nested_objects.length()
}

/// Check if the nested objects table is empty
public fun is_nested_objects_empty(main_object: &MainObject): bool {
    main_object.nested_objects.is_empty()
}

/// Create a sample main object with some nested objects and share it
public fun create_sample_object(ctx: &mut sui::tx_context::TxContext) {
    // Create main object
    let title = string::utf8(b"Sample Main Object");
    let mut numbers = std::vector::empty<u256>();
    std::vector::push_back(&mut numbers, 100u256);
    std::vector::push_back(&mut numbers, 200u256);
    std::vector::push_back(&mut numbers, 300u256);

    let mut main_object = MainObject {
        id: sui::object::new(ctx),
        title,
        numbers,
        nested_objects: object_table::new(ctx),
        version: 1,
    };

    // Create and add nested objects
    let mut nested_values_1 = std::vector::empty<u256>();
    std::vector::push_back(&mut nested_values_1, 10u256);
    std::vector::push_back(&mut nested_values_1, 20u256);
    let nested_obj_1 = new_nested_object(
        nested_values_1,
        string::utf8(b"First nested object"),
        ctx,
    );
    main_object.nested_objects.add(string::utf8(b"first"), nested_obj_1);

    let mut nested_values_2 = std::vector::empty<u256>();
    std::vector::push_back(&mut nested_values_2, 50u256);
    std::vector::push_back(&mut nested_values_2, 75u256);
    std::vector::push_back(&mut nested_values_2, 100u256);
    let nested_obj_2 = new_nested_object(
        nested_values_2,
        string::utf8(b"Second nested object"),
        ctx,
    );
    main_object.nested_objects.add(string::utf8(b"second"), nested_obj_2);

    // Share the main object
    sui::transfer::share_object(main_object);
}

/// Update a nested object's description
public fun update_nested_object_description(
    main_object: &mut MainObject,
    key: String,
    new_description: String,
) {
    let nested_obj = main_object.nested_objects.borrow_mut(key);
    nested_obj.description = new_description;
    main_object.version = main_object.version + 1;
}

/// Add a value to a nested object's values vector
public fun add_value_to_nested_object(
    main_object: &mut MainObject,
    key: String,
    value: u256,
) {
    let nested_obj = main_object.nested_objects.borrow_mut(key);
    std::vector::push_back(&mut nested_obj.values, value);
    main_object.version = main_object.version + 1;
}

const EInvalidVersion: u64 = 0x1;

/// Copy a MainObject creating a new instance with copied fields
/// Note: nested_objects requires keys to be specified due to ObjectTable limitations
public fun copy_object(
    original: &MainObject,
    keys_to_copy: vector<String>,
    version: u64,
    ctx: &mut sui::tx_context::TxContext,
): MainObject {
    assert!(version == original.version, EInvalidVersion);
    // Create new ObjectTable
    let mut new_nested_objects = object_table::new(ctx);

    // Copy each nested object for the specified keys
    let mut i = 0;
    while (i < std::vector::length(&keys_to_copy)) {
        let key = *std::vector::borrow(&keys_to_copy, i);
        if (original.nested_objects.contains(key)) {
            let original_nested = original.nested_objects.borrow(key);
            // Create a copy of the nested object with a new UID
            let copied_nested = NestedObject {
                id: sui::object::new(ctx),
                values: original_nested.values,
                description: original_nested.description,
            };
            new_nested_objects.add(key, copied_nested);
        };
        i = i + 1;
    };

    // Create and return new MainObject with copied fields
    MainObject {
        id: sui::object::new(ctx),
        title: original.title,
        numbers: original.numbers,
        nested_objects: new_nested_objects,
        version: original.version,
    }
}

/// Copy a MainObject with empty nested objects table (simpler version)
/// Use this when you don't need to copy the nested objects
public fun copy_object_basic(
    original: &MainObject,
    ctx: &mut sui::tx_context::TxContext,
): MainObject {
    MainObject {
        id: sui::object::new(ctx),
        title: original.title,
        numbers: original.numbers,
        nested_objects: object_table::new(ctx),
        version: original.version,
    }
}
