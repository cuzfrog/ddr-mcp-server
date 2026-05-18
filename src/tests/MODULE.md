# Module - Integration Tests

## fixtures.rs
This file should only serve data preparation for integration tests. Do not add any logic here.

## mock_xxx.rs
* pub fn mock_xxxx() to create a shared mock for testing.
* struct MockXxxxx implements the trait, contains necessary simple logic for testing.

This manual implementeation should be avoided if the mocking logic is simple. Try to use `mockall` crate for mocking dependencies directly in unit tests.

## workflow.rs
Integration test cases.